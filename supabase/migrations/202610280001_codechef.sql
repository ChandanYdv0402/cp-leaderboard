-- Run once in the Supabase SQL Editor after Clerk and per-platform migrations.
-- Apply the groups migration first if groups are wanted, or rerun this file after adding groups.
-- Safe to rerun. Existing accounts, snapshots, and group membership are preserved.
begin;

alter table public.accounts drop constraint if exists accounts_platform_check;
alter table public.accounts add constraint accounts_platform_check
  check (platform in ('codeforces', 'leetcode', 'codechef'));

-- RPC return columns change; replace their signatures together, without CASCADE.
drop function if exists public.group_leaderboard(uuid, integer);
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
  cc_points integer,
  cc_solved integer,
  cf_rating integer,
  lc_rating integer,
  cc_rating integer
)
language sql
stable
security invoker
set search_path = ''
as $$
  -- Existing snapshots lack cc_solved; coalesce keeps their previous scores unchanged.
  -- CodeChef contributes 10 points per solved problem. Ratings do not contribute points.
  with s as (
    select
      user_id, taken_at, cf_rating, lc_rating,
      (stats ->> 'cc_rating')::integer as cc_rating,
      10 * coalesce((stats ->> 'cc_solved')::integer, 0) as cc_points,
      coalesce((stats ->> 'cc_solved')::integer, 0) as cc_solved,
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
      case when bs.user_id is not null then bs.lc_solved when days is null then 0 else f.lc_solved end as lc_solved,
      case when bs.user_id is not null then bs.cc_points when days is null then 0 else f.cc_points end as cc_points,
      case when bs.user_id is not null then bs.cc_solved when days is null then 0 else f.cc_solved end as cc_solved
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
      greatest(c.cc_points - b.cc_points, 0) as cc_points,
      greatest(c.cc_solved - b.cc_solved, 0) as cc_solved,
      c.cc_rating,
      c.cf_rating,
      c.lc_rating
    from cur c
    join b on b.user_id = c.user_id
  )
  select
    p.name,
    d.cf_points + d.lc_points + d.cc_points,
    d.cf_solved + d.lc_solved + d.cc_solved,
    d.cf_points, d.cf_solved, d.lc_points, d.lc_solved, d.cc_points, d.cc_solved,
    d.cf_rating, d.lc_rating, d.cc_rating
  from d
  join public.profiles p on p.id = d.user_id
  order by 2 desc;
$$;

grant execute on function public.leaderboard(integer) to anon, authenticated;

-- Groups are optional. A project without group tables still gets global CodeChef rankings.
do $migration$
begin
  if to_regclass('public.groups') is not null and to_regclass('public.group_members') is not null then
    execute $function$
create or replace function public.group_leaderboard(target_group uuid, days integer default null)
returns table (name text, points integer, solved integer, cf_rating integer, lc_rating integer, cf_points integer, cf_solved integer, lc_points integer, lc_solved integer, cc_rating integer, cc_points integer, cc_solved integer, has_snapshot boolean)
language plpgsql stable security invoker set search_path = '' as $$
begin
  if not private.is_group_member(target_group) then
    raise exception 'Group not found or access denied.';
  end if;
  return query
    select p.name, coalesce(l.points, 0), coalesce(l.solved, 0), l.cf_rating, l.lc_rating,
      coalesce(l.cf_points, 0), coalesce(l.cf_solved, 0), coalesce(l.lc_points, 0), coalesce(l.lc_solved, 0),
      l.cc_rating, coalesce(l.cc_points, 0), coalesce(l.cc_solved, 0),
      l.name is not null
    from public.group_members m
    join public.profiles p on p.id = m.user_id
    left join public.leaderboard(days) l on l.name = p.name
    where m.group_id = target_group
    order by coalesce(l.points, 0) desc, coalesce(l.solved, 0) desc, p.name;
end;
$$;

$function$;
    revoke all on function public.group_leaderboard(uuid, integer) from public, anon, authenticated;
    grant execute on function public.group_leaderboard(uuid, integer) to authenticated;
  end if;
end;
$migration$;

notify pgrst, 'reload schema';
commit;
