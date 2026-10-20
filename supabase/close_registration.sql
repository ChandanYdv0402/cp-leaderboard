-- Close registration: signed-in users can no longer create a NEW profile.
-- Existing players can still edit theirs. Run once in the Supabase SQL Editor.
--
-- Why not just drop the insert policy? The site saves profiles with an upsert
-- (INSERT ... ON CONFLICT DO UPDATE), and Postgres checks INSERT policies even when
-- the row already exists. So the policy stays, but now only passes for a profile
-- that is already there.

begin;

drop policy if exists "users insert own profile" on public.profiles;

create policy "users insert own profile" on public.profiles
  for insert to authenticated
  with check (
    (select auth.jwt() ->> 'sub') = id
    and exists (select 1 from public.profiles p where p.id = profiles.id)
  );

commit;

-- To reopen registration later, run:
--
--   drop policy if exists "users insert own profile" on public.profiles;
--   create policy "users insert own profile" on public.profiles
--     for insert to authenticated with check ((select auth.jwt() ->> 'sub') = id);
--
-- and set REGISTRATION_OPEN = true in site/index.html.
