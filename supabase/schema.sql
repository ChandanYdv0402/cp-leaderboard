create table public.profiles (
  id text primary key, -- Clerk user id (JWT "sub")
  name text not null unique check (char_length(name) between 2 and 32),
  created_at timestamptz not null default now()
);

create table public.accounts (
  user_id text not null references public.profiles on delete cascade,
  platform text not null check (platform in ('codeforces', 'leetcode')),
  handle text not null check (handle ~ '^[A-Za-z0-9_.-]{1,40}$'),
  verified boolean not null default false,
  primary key (user_id, platform),
  unique (platform, handle)
);

create table public.snapshots (
  id bigint generated always as identity primary key,
  user_id text not null references public.profiles on delete cascade,
  taken_at timestamptz not null default now(),
  points integer not null,
  solved integer not null,
  cf_rating integer,
  lc_rating integer,
  stats jsonb not null
);
create index snapshots_user_taken_at_idx on public.snapshots (user_id, taken_at);

create table public.contests (
  platform text not null,
  name text not null,
  start timestamptz not null,
  duration integer not null,
  url text not null,
  primary key (platform, url)
);

alter table public.profiles enable row level security;
alter table public.accounts enable row level security;
alter table public.snapshots enable row level security;
alter table public.contests enable row level security;

create policy "public read profiles" on public.profiles
  for select using (true);
create policy "public read accounts" on public.accounts
  for select using (true);
create policy "public read snapshots" on public.snapshots
  for select using (true);
create policy "public read contests" on public.contests
  for select using (true);

create policy "users insert own profile" on public.profiles
  for insert to authenticated with check ((select auth.jwt() ->> 'sub') = id);
create policy "users update own profile" on public.profiles
  for update to authenticated using ((select auth.jwt() ->> 'sub') = id) with check ((select auth.jwt() ->> 'sub') = id);
create policy "users manage own accounts" on public.accounts
  for all to authenticated
  using ((select auth.jwt() ->> 'sub') = user_id)
  with check ((select auth.jwt() ->> 'sub') = user_id);

grant select on public.profiles, public.accounts, public.snapshots, public.contests to anon, authenticated;
grant insert, update on public.profiles to authenticated;
grant insert, update, delete on public.accounts to authenticated;
grant all on public.profiles, public.accounts, public.snapshots, public.contests to service_role;

create or replace function public.leaderboard(days integer default null)
returns table (
  name text,
  points integer,
  solved integer,
  cf_rating integer,
  lc_rating integer
)
language sql
stable
security invoker
set search_path = ''
as $$
  with cur as (
    select distinct on (s.user_id) s.*
    from public.snapshots s
    order by s.user_id, s.taken_at desc
  ),
  base as (
    select distinct on (s.user_id) s.*
    from public.snapshots s
    where days is not null
      and s.taken_at <= coalesce(
        (select max(taken_at) from public.snapshots) - make_interval(days => days),
        '-infinity'::timestamptz
      )
    order by s.user_id, s.taken_at desc
  ),
  first_snapshot as (
    select distinct on (s.user_id) s.*
    from public.snapshots s
    order by s.user_id, s.taken_at asc
  )
  select
    p.name,
    greatest(c.points - coalesce(b.points, case when days is null then 0 else f.points end), 0),
    greatest(c.solved - coalesce(b.solved, case when days is null then 0 else f.solved end), 0),
    c.cf_rating,
    c.lc_rating
  from cur c
  join public.profiles p on p.id = c.user_id
  left join base b on b.user_id = c.user_id
  left join first_snapshot f on f.user_id = c.user_id
  order by 2 desc;
$$;

grant execute on function public.leaderboard(integer) to anon, authenticated;