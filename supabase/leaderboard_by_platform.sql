-- Run once in the Supabase SQL Editor: leaderboard() now also returns per-platform points/solved.
-- The return type changes, so the old function must be dropped first.

drop function if exists public.leaderboard(integer);

create or replace function public.leaderboard(days integer default null)
returns table (
  name text,
  points integer,
  solved integer,
  cf_points integer,
  cf_solved integer,
  lc_points integer,
  lc_solved integer,
  cf_rating integer,
  lc_rating integer
)
language sql
stable
security invoker
set search_path = ''
as $$
  -- Per-platform totals derived from the stored Stats JSON (scoring: LC easy/medium/hard = 5/10/20).
  with s as (
    select
      user_id, taken_at, cf_rating, lc_rating,
      coalesce((stats ->> 'cf_points')::integer, 0) as cf_points,
      coalesce((stats ->> 'cf_solved')::integer, 0) as cf_solved,
      5 * coalesce((stats ->> 'lc_easy')::integer, 0)
        + 10 * coalesce((stats ->> 'lc_medium')::integer, 0)
        + 20 * coalesce((stats ->> 'lc_hard')::integer, 0) as lc_points,
      coalesce((stats ->> 'lc_easy')::integer, 0)
        + coalesce((stats ->> 'lc_medium')::integer, 0)
        + coalesce((stats ->> 'lc_hard')::integer, 0) as lc_solved
    from public.snapshots
  ),
  cur as (
    select distinct on (user_id) * from s order by user_id, taken_at desc
  ),
  base as (
    select distinct on (user_id) *
    from s
    where days is not null
      and taken_at <= coalesce(
        (select max(taken_at) from public.snapshots) - make_interval(days => days),
        '-infinity'::timestamptz
      )
    order by user_id, taken_at desc
  ),
  first_snapshot as (
    select distinct on (user_id) * from s order by user_id, taken_at asc
  ),
  -- Baseline: snapshot at/before the window start, else earliest (windowed) or zero (all-time).
  b as (
    select
      c.user_id,
      case when bs.user_id is not null then bs.cf_points when days is null then 0 else f.cf_points end as cf_points,
      case when bs.user_id is not null then bs.cf_solved when days is null then 0 else f.cf_solved end as cf_solved,
      case when bs.user_id is not null then bs.lc_points when days is null then 0 else f.lc_points end as lc_points,
      case when bs.user_id is not null then bs.lc_solved when days is null then 0 else f.lc_solved end as lc_solved
    from cur c
    left join base bs on bs.user_id = c.user_id
    left join first_snapshot f on f.user_id = c.user_id
  ),
  d as (
    select
      c.user_id,
      greatest(c.cf_points - b.cf_points, 0) as cf_points,
      greatest(c.cf_solved - b.cf_solved, 0) as cf_solved,
      greatest(c.lc_points - b.lc_points, 0) as lc_points,
      greatest(c.lc_solved - b.lc_solved, 0) as lc_solved,
      c.cf_rating,
      c.lc_rating
    from cur c
    join b on b.user_id = c.user_id
  )
  select
    p.name,
    d.cf_points + d.lc_points,
    d.cf_solved + d.lc_solved,
    d.cf_points, d.cf_solved, d.lc_points, d.lc_solved,
    d.cf_rating, d.lc_rating
  from d
  join public.profiles p on p.id = d.user_id
  order by 2 desc;
$$;

grant execute on function public.leaderboard(integer) to anon, authenticated;
