create table public.profiles (
  id uuid primary key references auth.users on delete cascade,
  name text not null unique check (char_length(name) between 2 and 32),
  created_at timestamptz not null default now()
);

create table public.accounts (
  user_id uuid not null references public.profiles on delete cascade,
  platform text not null check (platform in ('codeforces', 'leetcode')),
  handle text not null check (handle ~ '^[A-Za-z0-9_.-]{1,40}$'),
  verified boolean not null default false,
  primary key (user_id, platform),
  unique (platform, handle)
);

create table public.snapshots (
  id bigint generated always as identity primary key,
  user_id uuid not null references public.profiles on delete cascade,
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
  for insert to authenticated with check (auth.uid() = id);
create policy "users update own profile" on public.profiles
  for update to authenticated using (auth.uid() = id) with check (auth.uid() = id);
create policy "users manage own accounts" on public.accounts
  for all to authenticated
  using (auth.uid() = user_id)
  with check (auth.uid() = user_id);

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
-- Invitation-only groups (also available as an incremental migration).
-- Existing projects: run this file in the Supabase SQL Editor before deploying the UI.
begin;

create table public.groups (
  id uuid primary key default gen_random_uuid(),
  name text not null check (name = btrim(name) and char_length(name) between 2 and 60),
  owner_id uuid not null references public.profiles on delete cascade,
  created_at timestamptz not null default now()
);

create table public.group_members (
  group_id uuid not null references public.groups on delete cascade,
  user_id uuid not null references public.profiles on delete cascade,
  joined_at timestamptz not null default now(),
  primary key (group_id, user_id)
);
create index group_members_user_id_idx on public.group_members (user_id, group_id);

-- Separate from groups so members cannot retrieve invitation codes through table reads.
create table public.group_invites (
  group_id uuid primary key references public.groups on delete cascade,
  code text not null unique default replace(gen_random_uuid()::text, '-', '')
);

alter table public.groups enable row level security;
alter table public.group_members enable row level security;
alter table public.group_invites enable row level security;

create schema if not exists private;
grant usage on schema private to authenticated;

-- A definer helper avoids recursive membership policies. It only checks the caller.
create or replace function private.is_group_member(target_group uuid)
returns boolean language sql stable security definer set search_path = '' as $$
  select exists (
    select 1 from public.group_members m
    where m.group_id = target_group and m.user_id = (select auth.uid())
  );
$$;
revoke all on function private.is_group_member(uuid) from public, anon, authenticated;
grant execute on function private.is_group_member(uuid) to authenticated;

create policy "members read own groups" on public.groups
  for select to authenticated using (private.is_group_member(id));
create policy "members read group members" on public.group_members
  for select to authenticated using (private.is_group_member(group_id));

-- All writes, including joins, go through checked, atomic functions below.
revoke all on public.groups, public.group_members, public.group_invites from public, anon, authenticated;
grant select on public.groups, public.group_members to authenticated;
grant all on public.groups, public.group_members, public.group_invites to service_role;

create or replace function public.create_group(group_name text)
returns uuid language plpgsql security definer set search_path = '' as $$
declare
  caller uuid := auth.uid();
  created_group uuid;
begin
  if caller is null then raise exception 'Sign in first.'; end if;
  if not exists (select 1 from public.profiles where id = caller) then
    raise exception 'Save your profile before creating a group.';
  end if;
  if group_name is null or char_length(btrim(group_name)) not between 2 and 60 then
    raise exception 'Use a group name between 2 and 60 characters.';
  end if;
  insert into public.groups (name, owner_id) values (btrim(group_name), caller)
    returning id into created_group;
  insert into public.group_members (group_id, user_id) values (created_group, caller);
  insert into public.group_invites (group_id) values (created_group);
  return created_group;
end;
$$;

create or replace function public.join_group(invite_code text)
returns uuid language plpgsql security definer set search_path = '' as $$
declare
  caller uuid := auth.uid();
  target_group uuid;
  normalized_code text := lower(btrim(invite_code));
begin
  if caller is null then raise exception 'Sign in first.'; end if;
  if not exists (select 1 from public.profiles where id = caller) then
    raise exception 'Save your profile before joining a group.';
  end if;
  select i.group_id into target_group from public.group_invites i where i.code = normalized_code;
  -- Serialize joins with owner actions; recheck the code after taking the lock.
  perform 1 from public.groups g where g.id = target_group for update;
  if not found or not exists (
    select 1 from public.group_invites i where i.group_id = target_group and i.code = normalized_code
  ) then
    raise exception 'This invitation is invalid or has been replaced. Ask the owner for a new link.';
  end if;
  insert into public.group_members (group_id, user_id) values (target_group, caller)
    on conflict do nothing;
  return target_group;
end;
$$;

create or replace function public.group_invite(target_group uuid, rotate boolean default false)
returns text language plpgsql security definer set search_path = '' as $$
declare invitation text;
begin
  perform 1 from public.groups g where g.id = target_group and g.owner_id = auth.uid() for update;
  if not found then raise exception 'Only the group owner can manage invitations.'; end if;
  if rotate then
    update public.group_invites set code = replace(gen_random_uuid()::text, '-', '')
      where group_id = target_group;
  end if;
  select code into invitation from public.group_invites where group_id = target_group;
  return invitation;
end;
$$;

create or replace function public.leave_group(target_group uuid)
returns void language plpgsql security definer set search_path = '' as $$
declare group_owner uuid;
begin
  if auth.uid() is null then raise exception 'Sign in first.'; end if;
  select owner_id into group_owner from public.groups where id = target_group for update;
  if group_owner = auth.uid() then
    raise exception 'The owner must delete the group instead of leaving it.';
  end if;
  delete from public.group_members where group_id = target_group and user_id = auth.uid();
end;
$$;

create or replace function public.remove_group_member(target_group uuid, member_id uuid)
returns void language plpgsql security definer set search_path = '' as $$
begin
  perform 1 from public.groups g where g.id = target_group and g.owner_id = auth.uid() for update;
  if not found then raise exception 'Only the group owner can remove members.'; end if;
  if member_id = auth.uid() then raise exception 'The owner cannot be removed.'; end if;
  delete from public.group_members where group_id = target_group and user_id = member_id;
end;
$$;

create or replace function public.delete_group(target_group uuid)
returns void language plpgsql security definer set search_path = '' as $$
begin
  delete from public.groups where id = target_group and owner_id = auth.uid();
  if not found then raise exception 'Only the group owner can delete the group.'; end if;
end;
$$;

-- Keep the collector and global scoring unchanged; filter the same standings by membership.
-- Include new members before their first snapshot so the dashboard never hides them.
create or replace function public.group_leaderboard(target_group uuid, days integer default null)
returns table (name text, points integer, solved integer, cf_rating integer, lc_rating integer, has_snapshot boolean)
language plpgsql stable security invoker set search_path = '' as $$
begin
  if not private.is_group_member(target_group) then
    raise exception 'Group not found or access denied.';
  end if;
  return query
    select p.name, coalesce(l.points, 0), coalesce(l.solved, 0), l.cf_rating, l.lc_rating,
      l.name is not null
    from public.group_members m
    join public.profiles p on p.id = m.user_id
    left join public.leaderboard(days) l on l.name = p.name
    where m.group_id = target_group
    order by coalesce(l.points, 0) desc, coalesce(l.solved, 0) desc, p.name;
end;
$$;

revoke all on function public.create_group(text), public.join_group(text),
  public.group_invite(uuid, boolean), public.leave_group(uuid),
  public.remove_group_member(uuid, uuid), public.delete_group(uuid),
  public.group_leaderboard(uuid, integer) from public, anon, authenticated;
grant execute on function public.create_group(text), public.join_group(text),
  public.group_invite(uuid, boolean), public.leave_group(uuid),
  public.remove_group_member(uuid, uuid), public.delete_group(uuid),
  public.group_leaderboard(uuid, integer) to authenticated;

commit;
