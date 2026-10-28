import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { readFile, mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { once } from 'node:events';

const browserPath = process.env.CP_BROWSER || [
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  '/usr/bin/google-chrome', '/usr/bin/google-chrome-stable', '/usr/bin/chromium', '/usr/bin/chromium-browser',
].find(existsSync);
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

// The browser runs the actual page with deterministic Clerk/Supabase boundaries. SQL/RLS
// behavior is covered separately against PostgreSQL in groups.test.mjs.
function installMock() {
  const owner = { id: 'alice', primaryEmailAddress: { emailAddress: 'alice@gmail.com' }, fullName: 'Alice' };
  const initialGroups = [
    { id: 'group-a', name: 'Friends <club>', owner_id: 'alice' },
    { id: 'group-b', name: 'Study group', owner_id: 'bob' },
  ];
  const state = window.mock = {
    user: owner, groups: initialGroups, calls: [], charts: [], slowGroup: false, codechef: false,
    invitation: 'a'.repeat(32), authCallback: null,
    async signOut() { this.user = null; window.Clerk.user = null; this.authCallback(); },
    async signIn() { this.user = owner; window.Clerk.user = owner; this.authCallback(); },
  };
  const members = [
    { user_id: 'alice', profiles: { name: 'Alice' } },
    { user_id: 'bob', profiles: { name: 'Bob' } },
  ];
  const rows = [
    { name: 'Alice', points: 100, solved: 10, cf_points: 60, cf_solved: 6, lc_points: 40, lc_solved: 4, cc_points: 0, cc_solved: 0, cc_rating: null, cf_rating: 1400, lc_rating: null, has_snapshot: true },
    { name: 'Bob', points: 0, solved: 0, cf_points: 0, cf_solved: 0, lc_points: 0, lc_solved: 0, cc_points: 0, cc_solved: 0, cc_rating: null, cf_rating: null, lc_rating: null, has_snapshot: false },
  ];
  const profiles = [{ id: 'alice', name: 'Alice' }, { id: 'bob', name: 'Bob' }, { id: 'outsider', name: 'Global outsider' }];
  const snaps = ['alice', 'outsider'].flatMap(user_id => [0, 1].map(day => ({
    user_id, taken_at: new Date(Date.now() - (1 - day) * 864e5).toISOString(),
    points: 50 + 50 * day, solved: 5 + 5 * day, cf_rating: 1400, lc_rating: null,
    stats: { cf_points: 30 + 30 * day, cf_solved: 3 + 3 * day, lc_medium: 2 + 2 * day, cc_solved: 2 + 3 * day, cc_rating: 1723 },
  })));
  window.Clerk = {
    user: owner, session: { getToken: async () => 'test-token' },
    load: async () => {}, addListener(callback) { state.authCallback = callback; },
    signOut: () => state.signOut(),
  };
  window.Chart = class {
    static defaults = { font: {}, plugins: { tooltip: {} } };
    constructor(canvas, config) { state.charts.push({ id: canvas.id, config }); }
    destroy() {}
  };
  const api = {
    from(table) {
      state.calls.push({ table });
      const query = {
        select() { return this; }, eq() { return this; }, gte() { return this; },
        order() { return this; }, range() { return this; }, limit() { return this; },
        upsert(value) { state.calls.push({ table, upsert: value }); return this; }, delete() { return this; },
        maybeSingle() { return Promise.resolve({ data: table === 'profiles' ? { name: 'Alice' } : null }); },
        then(resolve, reject) {
          const contests = [{ platform: 'codechef', name: 'CodeChef Starters', start: new Date(Date.now() + 864e5).toISOString(), duration: 7200, url: 'https://www.codechef.com/START262' }];
          const data = { groups: state.groups.slice(), group_members: members.slice(), accounts: [], contests, profiles, snapshots: snaps }[table];
          return Promise.resolve({ data }).then(resolve, reject);
        },
      };
      return query;
    },
    async rpc(name, args) {
      state.calls.push({ rpc: name, args });
      if (name === 'leaderboard') return { data: [{ ...rows[0], name: 'Global outsider', points: 999 }] };
      if (name === 'group_leaderboard') {
        const factor = args.days === null ? 2 : 1;
        const result = rows.map((row, index) => {
          const cc_solved = state.codechef ? (index ? 8 : 5) : 0;
          return { ...row, points: (row.points + cc_solved * 10) * factor, solved: row.solved + cc_solved,
            cf_points: row.cf_points * factor, lc_points: row.lc_points * factor, cc_points: cc_solved * 10 * factor,
            cc_solved, cc_rating: state.codechef ? (index ? 1900 : 1723) : null, has_snapshot: state.codechef || row.has_snapshot };
        });
        if (state.slowGroup) await new Promise(resolve => setTimeout(resolve, 150));
        return { data: result };
      }
      if (name === 'create_group') {
        state.groups.push({ id: 'group-new', name: args.group_name, owner_id: 'alice' });
        return { data: 'group-new' };
      }
      if (name === 'join_group') {
        if (args.invite_code !== state.invitation) return { error: { message: 'This invitation is invalid or has been replaced.' } };
        state.groups.push({ id: 'group-joined', name: 'Invited club', owner_id: 'bob' });
        return { data: 'group-joined' };
      }
      if (name === 'group_invite') {
        if (args.rotate) state.invitation = 'b'.repeat(32);
        return { data: state.invitation };
      }
      if (name === 'leave_group' || name === 'delete_group') state.groups = state.groups.filter(group => group.id !== args.target_group);
      return { data: null };
    },
  };
  window.confirm = () => true;
  Object.defineProperty(navigator, 'clipboard', { value: { writeText: async () => { throw Error('Clipboard unavailable'); } } });
  window.createClient = () => api;
}

test('group dashboard works in a real browser', { skip: !browserPath && 'Set CP_BROWSER to a Chromium browser executable.', timeout: 45000 }, async t => {
  const temp = await mkdtemp(path.join(tmpdir(), 'cp-dashboard-'));
  const html = (await readFile(new URL('../site/index.html', import.meta.url), 'utf8'))
    .replaceAll('\r\n', '\n')
    .replace(/import \{ createClient \} from "[^"]+";/, 'const createClient = window.createClient;')
    .replace(/async function loadClerk\(\) \{[\s\S]*?\n\}\nfunction currentUser/, 'async function loadClerk() { return window.Clerk; }\nfunction currentUser')
    .replace(/<script defer src="https:\/\/cdnjs[^>]+><\/script>/, '')
    .replace('<script type="module">', `<script>(${installMock.toString()})()</script><script type="module">`);
  const server = createServer((_req, res) => { res.setHeader('Content-Type', 'text/html; charset=utf-8'); res.end(html); });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const browser = spawn(browserPath, [
    '--headless=new', '--disable-gpu', '--disable-dev-shm-usage', '--no-sandbox', '--no-first-run', '--no-default-browser-check',
    '--remote-debugging-port=0', `--user-data-dir=${temp}`, 'about:blank',
  ], { stdio: ['ignore', 'ignore', 'pipe'], windowsHide: true });
  let browserLog = '';
  browser.stderr.on('data', chunk => { browserLog = (browserLog + chunk).slice(-4000); });
  browser.on('error', error => { browserLog += error.message; });
  let socket;
  t.after(async () => {
    socket?.close();
    server.close();
    browser.kill();
    await Promise.race([once(browser, 'exit'), delay(2000)]);
    // Browser child processes can retain their temporary profile briefly on Windows.
    const resolvedTemp = path.resolve(temp);
    assert.ok(resolvedTemp.startsWith(path.resolve(tmpdir()) + path.sep) && path.basename(resolvedTemp).startsWith('cp-dashboard-'));
    await rm(resolvedTemp, { recursive: true, force: true, maxRetries: 8, retryDelay: 200 }).catch(() => {});
  });
  let port;
  for (let i = 0; i < 200; i++) {
    if (browser.exitCode !== null) break;
    try { port = (await readFile(path.join(temp, 'DevToolsActivePort'), 'utf8')).split('\n')[0]; break; } catch { await delay(100); }
  }
  assert.ok(port, `Browser must expose its debugging endpoint. ${browserLog}`);
  t.diagnostic('Browser debugging endpoint ready.');
  const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(5000) })).json();
  socket = new WebSocket(pages.find(page => page.type === 'page').webSocketDebuggerUrl);
  await once(socket, 'open', { signal: AbortSignal.timeout(5000) });
  t.diagnostic('Browser control connected.');
  let messageId = 0;
  const pending = new Map();
  const errors = [];
  socket.addEventListener('message', event => {
    const message = JSON.parse(event.data);
    if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails.text);
    if (message.id) {
      const callbacks = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) callbacks.reject(Error(message.error.message)); else callbacks.resolve(message.result);
    }
  });
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++messageId;
    const timer = setTimeout(() => { pending.delete(id); reject(Error(`Browser command timed out: ${method}`)); }, 8000);
    pending.set(id, { resolve: result => { clearTimeout(timer); resolve(result); }, reject: error => { clearTimeout(timer); reject(error); } });
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async expression => {
    const result = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  const waitFor = async expression => {
    for (let i = 0; i < 100; i++) { if (await evaluate(expression)) return; await delay(30); }
    assert.fail(`Timed out waiting for ${expression}`);
  };
  await send('Runtime.enable');
  t.diagnostic('Navigating to dashboard fixture.');
  await send('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/?invite=${'a'.repeat(32)}` });
  await waitFor(`document.getElementById('board').textContent.includes('Alice')`);

  await t.test('opens a group by default, renders safe names, pending stats, and period totals', async () => {
    assert.equal(await evaluate(`document.getElementById('dashboard-title').textContent`), 'Friends <club>');
    assert.equal(await evaluate(`document.getElementById('dashboard-title').children.length`), 0);
    assert.equal(await evaluate(`document.getElementById('group-points').textContent`), '100');
    assert.equal(await evaluate(`document.getElementById('member-count').textContent`), '2');
    assert.equal(await evaluate(`document.getElementById('board').textContent.includes('awaiting stats')`), true);
    assert.equal(await evaluate(`location.search.includes('invite')`), false);
    assert.equal(await evaluate(`localStorage.getItem('cp-leaderboard.pending-invite')`), 'a'.repeat(32));
    assert.equal(await evaluate(`document.getElementById('profile').open`), false);
    await evaluate(`document.querySelector('[data-k="all"]').click()`);
    await waitFor(`document.getElementById('group-points').textContent === '200'`);
  });

  await t.test('an invitation survives opening the sign-in destination in another tab', async () => {
    const { targetId } = await send('Target.createTarget', { url: `http://127.0.0.1:${server.address().port}/` });
    let otherSocket;
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      otherSocket = new WebSocket(targets.find(target => target.id === targetId).webSocketDebuggerUrl);
      await once(otherSocket, 'open', { signal: AbortSignal.timeout(5000) });
      const getCode = () => new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(Error('New tab did not respond.')), 5000);
        const listener = event => {
          const response = JSON.parse(event.data);
          if (response.id !== 1) return;
          clearTimeout(timer);
          otherSocket.removeEventListener('message', listener);
          resolve(response.result?.result?.value);
        };
        otherSocket.addEventListener('message', listener);
        otherSocket.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: {
          expression: `document.getElementById('invite-code')?.value`, returnByValue: true,
        } }));
      });
      let code;
      for (let i = 0; i < 100; i++) { code = await getCode(); if (code) break; await delay(30); }
      assert.equal(code, 'a'.repeat(32));
    } finally {
      otherSocket?.close();
      await send('Target.closeTarget', { targetId });
    }
  });

  await t.test('platform totals, ranks, progress, duels, and heatmaps stay inside the selected group', async () => {
    await waitFor(`document.getElementById('heatmap').textContent.includes('Alice')`);
    assert.equal(await evaluate(`document.getElementById('heatmap').textContent.includes('Global outsider')`), false);
    assert.equal(await evaluate(`document.getElementById('ranks').textContent.includes('Global outsider')`), false);
    assert.deepEqual(await evaluate(`Array.from(document.getElementById('duel-a').options).map(option => option.value)`), ['Alice', 'Bob']);
    await evaluate(`document.getElementById('platform').value = 'cf'; document.getElementById('platform').dispatchEvent(new Event('change'))`);
    assert.equal(await evaluate(`document.getElementById('group-points').textContent`), '120');
    await evaluate(`document.getElementById('platform').value = 'lc'; document.getElementById('platform').dispatchEvent(new Event('change'))`);
    assert.equal(await evaluate(`document.getElementById('group-points').textContent`), '80');
    await evaluate(`document.getElementById('platform').value = 'all'; document.getElementById('platform').dispatchEvent(new Event('change')); document.querySelector('[data-v="progress"]').click()`);
    await waitFor(`mock.charts.filter(chart => chart.id === 'chart').at(-1)?.config.type === 'line'`);
    assert.deepEqual(await evaluate(`mock.charts.filter(chart => chart.id === 'chart').at(-1).config.data.datasets.map(dataset => dataset.label)`), ['Alice']);
    assert.deepEqual(await evaluate(`mock.charts.filter(chart => chart.id === 'duel-chart').at(-1).config.data.datasets.map(dataset => dataset.label)`), ['Alice', 'Bob']);
    assert.equal(await evaluate(`document.getElementById('si-sub').textContent.includes('Registration is closed')`), true);
    await evaluate(`document.querySelector('[data-v="standings"]').click()`);
  });

  await t.test('CodeChef handles normalize and save without accepting another platform URL', async () => {
    assert.equal(await evaluate(`document.getElementById('codechef').value`), '');
    await evaluate(`document.getElementById('codechef').value = 'https://codeforces.com/profile/wrong'; document.getElementById('codechef').dispatchEvent(new Event('input')); document.getElementById('profile-form').requestSubmit()`);
    await waitFor(`document.getElementById('profile-message').textContent.includes('valid codechef')`);
    assert.equal(await evaluate(`document.getElementById('codechef-link').hidden`), true);
    await evaluate(`document.getElementById('codechef').value = 'https://www.codechef.com/users/chef_123/?tab=profile'; document.getElementById('codechef').dispatchEvent(new Event('blur'))`);
    assert.equal(await evaluate(`document.getElementById('codechef').value`), 'chef_123');
    assert.equal(await evaluate(`document.getElementById('codechef-link').href`), 'https://www.codechef.com/users/chef_123');
    await evaluate(`document.getElementById('profile-form').requestSubmit()`);
    await waitFor(`document.getElementById('profile-message').textContent.startsWith('Saved.')`);
    assert.deepEqual(await evaluate(`mock.calls.find(call => call.table === 'accounts' && call.upsert?.platform === 'codechef').upsert`),
      { user_id: 'alice', platform: 'codechef', handle: 'chef_123', verified: false });
  });

  await t.test('CodeChef filters, ratings, charts, rank cards, heatmaps, and contests render correctly', async () => {
    await evaluate(`mock.codechef = true; document.querySelector('[data-k="week"]').click()`);
    await waitFor(`document.getElementById('group-points').textContent === '230'`);
    await evaluate(`document.getElementById('platform').value = 'cc'; document.getElementById('platform').dispatchEvent(new Event('change'))`);
    assert.equal(await evaluate(`document.getElementById('group-points').textContent`), '130');
    assert.equal(await evaluate(`document.getElementById('group-solved').textContent`), '13');
    assert.equal(await evaluate(`document.getElementById('cf-head').hidden && document.getElementById('lc-head').hidden && !document.getElementById('cc-head').hidden`), true);
    assert.equal(await evaluate(`document.getElementById('board').firstElementChild.textContent.includes('Bob')`), true);
    assert.equal(await evaluate(`document.getElementById('board').textContent.includes('1900')`), true);
    assert.equal(await evaluate(`document.getElementById('ranks').textContent.includes('CC 4 stars')`), true);
    assert.equal(await evaluate(`document.getElementById('heatmap').textContent.includes('CodeChef') && !document.getElementById('heatmap').textContent.includes('Global outsider')`), true);
    assert.deepEqual(await evaluate(`mock.charts.filter(chart => chart.id === 'chart').at(-1).config.data.datasets.map(dataset => dataset.label)`), ['CodeChef']);
    await evaluate(`document.querySelector('[data-v="progress"]').click()`);
    await waitFor(`mock.charts.filter(chart => chart.id === 'chart').at(-1)?.config.type === 'line'`);
    assert.deepEqual(await evaluate(`mock.charts.filter(chart => chart.id === 'chart').at(-1).config.data.datasets[0].data.map(point => point.y)`), [0, 30]);
    assert.equal(await evaluate(`document.querySelector('#contests .tag.cc').textContent`), 'CC');
    assert.equal(await evaluate(`document.getElementById('countdown').textContent.includes('CodeChef Starters')`), true);
    assert.equal(await evaluate(`!!document.querySelector('#calendar .marks .cc')`), true);
    await evaluate(`mock.codechef = false; document.getElementById('platform').value = 'all'; document.getElementById('platform').dispatchEvent(new Event('change')); document.querySelector('[data-v="standings"]').click(); document.querySelector('[data-k="week"]').click()`);
    await waitFor(`document.getElementById('group-points').textContent === '100'`);
  });

  await t.test('global rankings remain selected when an older group response arrives', async () => {
    await evaluate(`mock.slowGroup = true; document.querySelector('[data-k="week"]').click(); document.getElementById('global-rankings').click()`);
    await waitFor(`document.getElementById('board').textContent.includes('Global outsider')`);
    await delay(220);
    assert.equal(await evaluate(`document.getElementById('board').textContent.includes('Global outsider')`), true);
    assert.equal(await evaluate(`document.getElementById('group-dashboard').hidden`), true);
    await evaluate(`mock.slowGroup = false; document.getElementById('dashboard-select').value = 'group-b'; document.getElementById('dashboard-select').dispatchEvent(new Event('change'))`);
    await waitFor(`document.getElementById('dashboard-title').textContent === 'Study group'`);
    assert.equal(await evaluate(`document.getElementById('owner-tools').hidden`), true);
    assert.equal(await evaluate(`document.getElementById('leave-group').hidden`), false);
  });

  await t.test('create, clipboard fallback, invite rotation, join, and leave update the dashboard', async () => {
    await evaluate(`document.getElementById('group-name').value = 'New club'; document.getElementById('create-group-form').requestSubmit()`);
    await waitFor(`document.getElementById('dashboard-title').textContent === 'New club'`);
    await evaluate(`document.getElementById('copy-invite').click()`);
    await waitFor(`!document.getElementById('invite-link-label').hidden`);
    assert.match(await evaluate(`document.getElementById('invite-link').value`), /\?invite=a{32}$/);
    await evaluate(`document.getElementById('rotate-invite').click()`);
    await waitFor(`document.getElementById('invite-link').value.endsWith('b'.repeat(32))`);
    await evaluate(`document.getElementById('invite-code').value = 'bad'; document.getElementById('join-group-form').requestSubmit()`);
    await waitFor(`document.getElementById('group-message').textContent.includes('32-character')`);
    await evaluate(`document.getElementById('invite-code').value = 'a'.repeat(32); document.getElementById('join-group-form').requestSubmit()`);
    await waitFor(`document.getElementById('group-message').textContent.includes('invalid or has been replaced')`);
    await evaluate(`document.getElementById('invite-code').value = 'https://example.com/?invite=' + 'b'.repeat(32); document.getElementById('join-group-form').requestSubmit()`);
    await waitFor(`document.getElementById('dashboard-title').textContent === 'Invited club'`);
    assert.equal(await evaluate(`localStorage.getItem('cp-leaderboard.pending-invite')`), null);
    await evaluate(`document.getElementById('leave-group').click()`);
    await waitFor(`document.getElementById('group-message').textContent === 'You left the group.'`);
    assert.equal(await evaluate(`Array.from(document.getElementById('dashboard-select').options).some(o => o.text === 'Invited club')`), false);
  });

  await t.test('mobile layout fits, sign-out clears private UI, and global preference persists', async () => {
    await send('Emulation.setDeviceMetricsOverride', { width: 320, height: 800, deviceScaleFactor: 1, mobile: true });
    const narrowWidth = await evaluate(`document.documentElement.scrollWidth`);
    assert.ok(narrowWidth <= 320, `Page width ${narrowWidth} exceeds the 320px viewport.`);
    await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
    assert.equal(await evaluate(`document.documentElement.scrollWidth <= 390`), true);
    if (process.env.CP_QA_SCREENSHOT) {
      const screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
      await writeFile(process.env.CP_QA_SCREENSHOT, Buffer.from(screenshot.data, 'base64'));
    }
    await evaluate(`document.getElementById('global-rankings').click(); mock.signOut()`);
    await waitFor(`document.getElementById('dashboard-select').disabled && document.getElementById('profile').hidden`);
    assert.equal(await evaluate(`document.getElementById('member-list').children.length`), 0);
    assert.equal(await evaluate(`document.getElementById('invite-link').value`), '');
    assert.equal(await evaluate(`document.getElementById('create-group').disabled`), true);
    await evaluate(`mock.signIn()`);
    await waitFor(`!document.getElementById('profile').hidden && !document.getElementById('dashboard-select').disabled`);
    assert.equal(await evaluate(`document.getElementById('dashboard-title').textContent`), 'Global rankings');
    assert.deepEqual(errors, []);
  });
});
