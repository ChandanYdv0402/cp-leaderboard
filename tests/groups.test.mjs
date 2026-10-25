import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { PGlite } from '@electric-sql/pglite';

const schema = await readFile(new URL('../supabase/schema.sql', import.meta.url), 'utf8');
const migration = await readFile(new URL('../supabase/migrations/202610250001_groups.sql', import.meta.url), 'utf8');
const alice = '00000000-0000-0000-0000-000000000001';
const bob = '00000000-0000-0000-0000-000000000002';
const outsider = '00000000-0000-0000-0000-000000000003';
const noProfile = '00000000-0000-0000-0000-000000000004';

test('fresh schema includes the exact incremental groups migration', () => {
  assert.ok(schema.endsWith(migration));
});

test('invitation-only groups enforce membership and ownership in PostgreSQL', async t => {
  const db = new PGlite();
  t.after(() => db.close());
  // Minimal Supabase auth contract; PostgreSQL itself executes the real RLS and RPCs.
  await db.exec(`
    create role anon;
    create role authenticated;
    create role service_role bypassrls;
    create schema auth;
    create table auth.users (id uuid primary key);
    create function auth.uid() returns uuid language sql stable as $$
      select nullif(current_setting('request.jwt.claim.sub', true), '')::uuid;
    $$;
    grant usage on schema auth to anon, authenticated;
  `);
  // Exercise the upgrade path rather than only installing the complete new schema.
  const baseSchema = schema.split('-- Invitation-only groups (also available as an incremental migration).')[0];
  await db.exec(baseSchema);
  // Supabase can give new public tables/functions API grants by default. The migration
  // must explicitly revoke these, particularly on invitation codes and write APIs.
  await db.exec(`
    alter default privileges in schema public grant all on tables to anon, authenticated;
    alter default privileges in schema public grant execute on functions to anon, authenticated;
  `);
  await db.exec(migration);
  await db.exec(`
    insert into auth.users values ('${alice}'), ('${bob}'), ('${outsider}'), ('${noProfile}');
    insert into public.profiles(id, name) values ('${alice}', 'Alice'), ('${bob}', 'Bob'), ('${outsider}', 'Outsider');
    insert into public.snapshots(user_id,taken_at,points,solved,stats) values
      ('${alice}', '2026-10-01', 100, 10, '{}'),
      ('${alice}', '2026-10-25', 200, 20, '{}'),
      ('${outsider}', '2026-10-25', 900, 90, '{}');
  `);
  const as = async user => {
    await db.exec('reset role');
    await db.query("select set_config('request.jwt.claim.sub', $1, false)", [user || '']);
    await db.exec(`set role ${user ? 'authenticated' : 'anon'}`);
  };
  const scalar = async (sql, args = []) => Object.values((await db.query(sql, args)).rows[0])[0];
  let group, invitation, otherGroup;

  await t.test('creation is atomic, validated, and requires a saved profile', async () => {
    await as(noProfile);
    await assert.rejects(db.query("select public.create_group('Club')"), /Save your profile/);
    await as(alice);
    await assert.rejects(db.query("select public.create_group(' ')"), /between 2 and 60/);
    await assert.rejects(db.query('select public.create_group(null)'), /between 2 and 60/);
    group = await scalar("select public.create_group('  Friends  ')");
    assert.equal(await scalar('select name from public.groups where id = $1', [group]), 'Friends');
    assert.equal(await scalar('select count(*) from public.group_members'), 1);
    invitation = await scalar('select public.group_invite($1)', [group]);
    assert.match(invitation, /^[0-9a-f]{32}$/);
    await assert.rejects(db.query('select * from public.group_invites'), /permission denied/);
  });

  await t.test('outsiders cannot discover groups, members, or invitations, or forge membership', async () => {
    await as(outsider);
    assert.equal(await scalar('select count(*) from public.groups'), 0);
    assert.equal(await scalar('select count(*) from public.group_members'), 0);
    await assert.rejects(db.query('select * from public.group_invites'), /permission denied/);
    await assert.rejects(db.query('select * from public.group_leaderboard($1)', [group]), /access denied/);
    await assert.rejects(db.query('select public.group_invite($1)', [group]), /Only the group owner/);
    await assert.rejects(db.query('select public.group_invite($1, true)', [group]), /Only the group owner/);
    await assert.rejects(db.query('insert into public.group_members(group_id,user_id) values ($1,$2)', [group, outsider]), /permission denied/);
    await assert.rejects(db.query('update public.groups set owner_id = $1 where id = $2', [outsider, group]), /permission denied/);
    await assert.rejects(db.query('delete from public.groups where id = $1', [group]), /permission denied/);
    await assert.rejects(db.query('select public.delete_group($1)', [group]), /Only the group owner/);
    await assert.rejects(db.query('select public.join_group($1)', ['f'.repeat(32)]), /invalid or has been replaced/);
    otherGroup = await scalar("select public.create_group('Other club')");
  });

  await t.test('invites are reusable, joins are idempotent, and new members appear without snapshots', async () => {
    await as(noProfile);
    await assert.rejects(db.query('select public.join_group($1)', [invitation]), /Save your profile/);
    await as(bob);
    assert.equal(await scalar('select public.join_group($1)', [` ${invitation.toUpperCase()} `]), group);
    assert.equal(await scalar('select public.join_group($1)', [invitation]), group);
    assert.equal(await scalar('select count(*) from public.groups'), 1);
    assert.equal(await scalar('select count(*) from public.group_members'), 2);
    const weekly = (await db.query('select * from public.group_leaderboard($1, 7)', [group])).rows;
    assert.deepEqual(weekly.map(r => [r.name, r.points, r.solved, r.has_snapshot]), [
      ['Alice', 100, 10, true], ['Bob', 0, 0, false],
    ]);
    assert.equal((await db.query('select * from public.group_leaderboard($1, null)', [group])).rows[0].points, 200);
    await assert.rejects(db.query('select * from public.group_leaderboard($1)', [otherGroup]), /access denied/);
    await assert.rejects(db.query('select public.remove_group_member($1, $2)', [group, alice]), /Only the group owner/);
    await assert.rejects(db.query('select public.group_invite($1)', [group]), /Only the group owner/);
  });

  await t.test('owner rotation invalidates old invitations while retaining existing members', async () => {
    await as(alice);
    const replacement = await scalar('select public.group_invite($1, true)', [group]);
    assert.notEqual(replacement, invitation);
    await as(outsider);
    await assert.rejects(db.query('select public.join_group($1)', [invitation]), /invalid or has been replaced/);
    invitation = replacement;
    await as(bob);
    assert.equal(await scalar('select count(*) from public.groups'), 1);
    await db.query('select public.leave_group($1)', [group]);
    assert.equal(await scalar('select count(*) from public.groups'), 0);
    await assert.rejects(db.query('select * from public.group_leaderboard($1)', [group]), /access denied/);
    await db.query('select public.join_group($1)', [invitation]);
  });

  await t.test('removal revokes access, owner cannot be removed, deletion cascades', async () => {
    await as(alice);
    await assert.rejects(db.query('select public.leave_group($1)', [group]), /owner must delete/);
    await assert.rejects(db.query('select public.remove_group_member($1,$2)', [group, alice]), /owner cannot be removed/);
    await db.query('select public.remove_group_member($1,$2)', [group, bob]);
    await as(bob);
    assert.equal(await scalar('select count(*) from public.group_members'), 0);
    await assert.rejects(db.query('select * from public.group_leaderboard($1)', [group]), /access denied/);
    await as(alice);
    await db.query('select public.delete_group($1)', [group]);
    assert.equal(await scalar('select count(*) from public.groups'), 0);
    await as(bob);
    await assert.rejects(db.query('select public.join_group($1)', [invitation]), /invalid or has been replaced/);
    await db.exec('reset role');
    assert.equal(await scalar('select count(*) from public.group_invites where group_id = $1', [group]), 0);
    assert.equal(await scalar('select count(*) from public.group_members where group_id = $1', [group]), 0);
  });

  await t.test('anonymous visitors retain global rankings but have no access to groups', async () => {
    await as(null);
    assert.deepEqual((await db.query('select name from public.leaderboard(null)')).rows.map(r => r.name), ['Outsider', 'Alice']);
    await assert.rejects(db.query('select * from public.groups'), /permission denied/);
    await assert.rejects(db.query('select * from public.group_members'), /permission denied/);
    await assert.rejects(db.query('select * from public.group_invites'), /permission denied/);
    await assert.rejects(db.query("select public.create_group('Club')"), /permission denied/);
    await assert.rejects(db.query('select public.join_group($1)', [invitation]), /permission denied/);
    await assert.rejects(db.query('select * from public.group_leaderboard($1)', [otherGroup]), /permission denied/);
  });
});
