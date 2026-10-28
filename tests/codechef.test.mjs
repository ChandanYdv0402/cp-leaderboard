import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { PGlite } from '@electric-sql/pglite';

const schema = await readFile(new URL('../supabase/schema.sql', import.meta.url), 'utf8');
const codechef = await readFile(new URL('../supabase/migrations/202610280001_codechef.sql', import.meta.url), 'utf8');
const groups = await readFile(new URL('../supabase/migrations/202610250001_groups.sql', import.meta.url), 'utf8');
const legacySchema = schema.split('-- Invitation-only groups (also available as an incremental migration).')[0];
const auth = `
  create role anon;
  create role authenticated;
  create role service_role bypassrls;
  create schema auth;
  create function auth.jwt() returns jsonb language sql stable as $$
    select jsonb_build_object('sub', nullif(current_setting('request.jwt.claim.sub', true), ''));
  $$;
  grant usage on schema auth to anon, authenticated;
`;

test('fresh installs include the exact CodeChef upgrade', async t => {
  assert.ok(schema.endsWith(codechef));
  const db = new PGlite();
  t.after(() => db.close());
  await db.exec(auth);
  await db.exec(schema);
  const result = await db.query('select * from public.leaderboard(null)');
  assert.ok(result.fields.some(field => field.name === 'cc_rating'));
  assert.ok(result.fields.some(field => field.name === 'cc_solved'));
});

test('CodeChef upgrades existing databases, preserves history, and supports optional groups', async t => {
  const db = new PGlite();
  t.after(() => db.close());
  await db.exec(auth);
  await db.exec(legacySchema);
  await db.exec(`
    insert into public.profiles(id,name) values ('user_alice','Alice'), ('user_bob','Bob');
    insert into public.snapshots(user_id,taken_at,points,solved,stats) values
      ('user_alice','2026-10-01',100,10,'{"cf_points":100,"cf_solved":10}'),
      ('user_bob','2026-10-28',20,2,'{"cf_points":20,"cf_solved":2}');
  `);
  const as = async user => {
    await db.exec('reset role');
    await db.query("select set_config('request.jwt.claim.sub',$1,false)", [user || '']);
    await db.exec(`set role ${user ? 'authenticated' : 'anon'}`);
  };

  await t.test('migration works before groups are installed and maintains account RLS', async () => {
    await db.exec(codechef);
    await as('user_alice');
    await db.query("insert into public.accounts(user_id,platform,handle) values ('user_alice','codechef','chef_alice')");
    await assert.rejects(db.query("insert into public.accounts(user_id,platform,handle) values ('user_bob','codechef','chef_bob')"), /row-level security/);
    await assert.rejects(db.query("insert into public.accounts(user_id,platform,handle) values ('user_alice','unsupported','chef_alice')"), /check constraint/);
    await as(null);
    assert.equal((await db.query("select cc_points from public.leaderboard(null) where name='Alice'")).rows[0].cc_points, 0);
    await db.exec('reset role');
  });

  await t.test('windowed and all-time totals include CodeChef while old snapshots stay unchanged', async () => {
    await db.exec(`
      insert into public.snapshots(user_id,taken_at,points,solved,stats) values
        ('user_alice','2026-10-21',260,26,'{"cf_points":150,"cf_solved":15,"lc_medium":1,"cc_solved":10,"cc_rating":1500}'),
        ('user_alice','2026-10-28',370,37,'{"cf_points":200,"cf_solved":20,"lc_medium":3,"cc_solved":14,"cc_rating":1650}');
    `);
    await as(null);
    const weekly = (await db.query("select * from public.leaderboard(7) where name='Alice'")).rows[0];
    assert.deepEqual([weekly.points, weekly.solved, weekly.cc_points, weekly.cc_solved, weekly.cc_rating], [110, 11, 40, 4, 1650]);
    const all = (await db.query("select * from public.leaderboard(null) where name='Alice'")).rows[0];
    assert.deepEqual([all.points, all.solved, all.cc_points, all.cc_solved], [370, 37, 140, 14]);
    const old = (await db.query("select * from public.leaderboard(null) where name='Bob'")).rows[0];
    assert.deepEqual([old.points, old.solved, old.cc_points, old.cc_solved, old.cc_rating], [20, 2, 0, 0, null]);
    const first = (await db.query("select points,solved,stats from public.snapshots where user_id='user_alice' order by taken_at limit 1")).rows[0];
    assert.deepEqual(first, { points: 100, solved: 10, stats: { cf_points: 100, cf_solved: 10 } });
    await db.exec('reset role');
  });

  await t.test('rerunning after adding groups preserves membership and protects expanded group standings', async () => {
    await db.exec(groups);
    await as('user_alice');
    const group = (await db.query("select public.create_group('CodeChef club') as id")).rows[0].id;
    await db.exec('reset role');
    await db.exec(codechef);
    await db.exec(codechef); // Repeated setup must not delete records or widen access.
    await as('user_alice');
    const row = (await db.query('select * from public.group_leaderboard($1,7)', [group])).rows[0];
    assert.deepEqual([row.name, row.cc_solved, row.cc_points, row.cc_rating], ['Alice', 4, 40, 1650]);
    assert.equal((await db.query('select count(*) from public.accounts')).rows[0].count, 1);
    await as('user_bob');
    await assert.rejects(db.query('select * from public.group_leaderboard($1,7)', [group]), /access denied/);
    await as(null);
    await assert.rejects(db.query('select * from public.group_leaderboard($1,7)', [group]), /permission denied/);
    assert.equal((await db.query('select count(*) from public.snapshots')).rows[0].count, 4);
  });
});
