-- One-off migration from Supabase Auth to Clerk (Supabase third-party auth).
-- Run once in the Supabase SQL Editor on a project created from the old schema.sql.
-- Clerk user ids are text ("user_2abc..."), and RLS reads them from the JWT "sub" claim.
-- Existing profiles keep their old Supabase uuid as text; they become orphans once everyone re-signs up with Clerk.

begin;

drop policy "users insert own profile" on public.profiles;
drop policy "users update own profile" on public.profiles;
drop policy "users manage own accounts" on public.accounts;

alter table public.accounts drop constraint accounts_user_id_fkey;
alter table public.snapshots drop constraint snapshots_user_id_fkey;
alter table public.profiles drop constraint profiles_id_fkey;   -- the reference to auth.users

alter table public.profiles alter column id type text using id::text;
alter table public.accounts alter column user_id type text using user_id::text;
alter table public.snapshots alter column user_id type text using user_id::text;

alter table public.accounts add constraint accounts_user_id_fkey
  foreign key (user_id) references public.profiles on delete cascade;
alter table public.snapshots add constraint snapshots_user_id_fkey
  foreign key (user_id) references public.profiles on delete cascade;

create policy "users insert own profile" on public.profiles
  for insert to authenticated with check ((select auth.jwt() ->> 'sub') = id);
create policy "users update own profile" on public.profiles
  for update to authenticated
  using ((select auth.jwt() ->> 'sub') = id)
  with check ((select auth.jwt() ->> 'sub') = id);
create policy "users manage own accounts" on public.accounts
  for all to authenticated
  using ((select auth.jwt() ->> 'sub') = user_id)
  with check ((select auth.jwt() ->> 'sub') = user_id);

commit;
