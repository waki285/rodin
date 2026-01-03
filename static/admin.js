const toArrayBuffer = (b64) => Uint8Array.from(atob(b64.replace(/-/g, '+').replace(/_/g, '/')), (c) => c.charCodeAt(0)).buffer;
const fromArrayBuffer = (buf) => btoa(String.fromCharCode(...new Uint8Array(buf))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');

const $ = (id) => document.getElementById(id);
const authStatus = $('auth-status');
const loginPanel = $('login-panel');
const registerPanel = $('register-panel');
const actions = $('actions');
const logEl = $('log');

async function fetchJSON(path, body) {
  const res = await fetch(path, {
    method: body ? 'POST' : 'GET',
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || res.statusText);
  return data;
}

async function refreshStatus() {
  const s = await fetchJSON('/__admin/status');
  authStatus.textContent = s.logged_in ? 'ログイン済み' : '未ログイン';
  authStatus.className = 'chip ' + (s.logged_in ? 'ok' : 'warn');
  loginPanel.classList.toggle('hidden', s.logged_in);
  actions.classList.toggle('hidden', !s.logged_in);
  registerPanel.classList.toggle('hidden', s.has_credential);
}

async function startRegister() {
  $('register-output').textContent = '';
  // POST でないとサーバーが 405 を返す
  const { options, challenge_b64 } = await fetchJSON('/__admin/passkey/register/options', {});
  const pubKey = options.publicKey;
  pubKey.challenge = toArrayBuffer(pubKey.challenge);
  pubKey.user.id = toArrayBuffer(pubKey.user.id);
  if (pubKey.excludeCredentials) {
    pubKey.excludeCredentials = pubKey.excludeCredentials.map((c) => ({ ...c, id: toArrayBuffer(c.id) }));
  }
  const cred = await navigator.credentials.create({ publicKey: pubKey });
  const att = {
    id: cred.id,
    rawId: fromArrayBuffer(cred.rawId),
    type: cred.type,
    response: {
      clientDataJSON: fromArrayBuffer(cred.response.clientDataJSON),
      attestationObject: fromArrayBuffer(cred.response.attestationObject),
    },
  };
  const result = await fetchJSON('/__admin/passkey/register/finish', { credential: att, challenge: challenge_b64 });
  $('register-output').textContent = '環境変数に設定: ' + result.env_value + '\n再起動後にログインしてください';
}

async function startLogin() {
  $('login-hint').textContent = '';
  // POST でないとサーバーが 405 を返す
  const { options, challenge_b64 } = await fetchJSON('/__admin/passkey/login/options', {});
  const pubKey = options.publicKey;
  pubKey.challenge = toArrayBuffer(pubKey.challenge);
  if (pubKey.allowCredentials) {
    pubKey.allowCredentials = pubKey.allowCredentials.map((c) => ({ ...c, id: toArrayBuffer(c.id) }));
  }
  const cred = await navigator.credentials.get({ publicKey: pubKey });
  const assertion = {
    id: cred.id,
    rawId: fromArrayBuffer(cred.rawId),
    type: cred.type,
    response: {
      clientDataJSON: fromArrayBuffer(cred.response.clientDataJSON),
      authenticatorData: fromArrayBuffer(cred.response.authenticatorData),
      signature: fromArrayBuffer(cred.response.signature),
      userHandle: cred.response.userHandle ? fromArrayBuffer(cred.response.userHandle) : null,
    },
  };
  await fetchJSON('/__admin/passkey/login/finish', { credential: assertion, challenge: challenge_b64 });
  await refreshStatus();
}

async function runBuild() {
  logEl.textContent = '実行中...';
  const payload = {
    opensearch: true,
    reset_os: $('reset-os').checked,
    skip_markdown: $('skip-md').checked,
  };
  const res = await fetchJSON('/__admin/api/run', payload);
  if (res.success) {
    logEl.textContent = res.log || 'done';
  } else {
    logEl.textContent = '❌ エラー:\n' + (res.error || 'unknown error') + '\n\n' + res.log;
  }
}

async function doReload() {
  logEl.textContent = 'リロード中...';
  // reload は POST のみ許可しているので空ボディで POST する
  const res = await fetchJSON('/__admin/api/reload', {});
  logEl.textContent = res.message || 'reloaded';
}

async function doGitPull() {
  logEl.textContent = 'Git Pull 実行中...';
  const res = await fetchJSON('/__admin/api/git-pull', {});
  if (res.success) {
    logEl.textContent = res.log || 'done';
  } else {
    logEl.textContent = '❌ エラー:\n' + (res.error || 'unknown error');
  }
}

async function doFontSubset() {
  logEl.textContent = 'フォントサブセット実行中...';
  const res = await fetchJSON('/__admin/api/font-subset', {});
  if (res.success) {
    logEl.textContent = res.log || 'done';
  } else {
    logEl.textContent = '❌ エラー:\n' + (res.error || 'unknown error');
  }
}

async function doPurgeCache() {
  logEl.textContent = 'キャッシュパージ実行中...';
  const res = await fetchJSON('/__admin/api/purge-cache', {});
  if (res.success) {
    logEl.textContent = res.log || 'done';
  } else {
    logEl.textContent = '❌ エラー:\n' + (res.error || 'unknown error');
  }
}

$('register-btn').onclick = () => startRegister().catch((e) => ($('register-output').textContent = e.message));
$('login-btn').onclick = () => startLogin().catch((e) => ($('login-hint').textContent = e.message));
$('run-build').onclick = () => runBuild().catch((e) => (logEl.textContent = e.message));
$('reload-btn').onclick = () => doReload().catch((e) => (logEl.textContent = e.message));
$('git-pull-btn').onclick = () => doGitPull().catch((e) => (logEl.textContent = e.message));
$('font-subset-btn').onclick = () => doFontSubset().catch((e) => (logEl.textContent = e.message));
$('purge-cache-btn').onclick = () => doPurgeCache().catch((e) => (logEl.textContent = e.message));

refreshStatus().catch(console.error);
