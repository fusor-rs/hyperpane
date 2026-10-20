// Injected by the consumer harness. Exercises the page, reloads once at the
// route it navigated to, then posts everything it observed to the host.
(() => {
  if (window.top !== window) return;
  const SAVED = 'hyperpane-probe';
  const DEADLINE_MS = 15000;
  const errors = [];
  for (const level of ['error', 'warn']) {
    const original = console[level].bind(console);
    console[level] = (...args) => {
      errors.push(`${level}: ${args.map(String).join(' ')}`);
      original(...args);
    };
  }
  window.addEventListener('error', (event) => errors.push(`error event: ${event.message}`));
  window.addEventListener('unhandledrejection', (event) =>
    errors.push(`unhandled rejection: ${String(event.reason)}`));

  const text = (selector) => document.querySelector(selector)?.textContent ?? null;
  const settled = async (read, done) => {
    const deadline = performance.now() + DEADLINE_MS;
    for (;;) {
      const value = read();
      if (done(value) || performance.now() > deadline) return value;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
  };
  const hydrated = () => settled(() => text('#clicks'), (value) => value !== 'Clicks: ');
  const loaded = (selector) => settled(() => text(selector), (value) => !value?.endsWith('pending'));
  const button = () => document.querySelector('#clicks');
  const status = (path) => fetch(path).then((response) => response.status, String);

  const exercise = async () => {
    const environment = {
      href: location.href,
      isSecureContext: window.isSecureContext,
      userAgent: navigator.userAgent,
      wasm: typeof WebAssembly,
      crossOriginIsolated: self.crossOriginIsolated,
      sharedArrayBuffer: typeof SharedArrayBuffer,
    };
    const before = { text: await hydrated(), pressed: button().getAttribute('aria-pressed') };
    button().click();
    const clicked = await settled(() => text('#clicks'), (value) => value !== before.text);
    const lengthBefore = history.length;
    const pageBefore = text('#page');
    document.querySelector('#details').click();
    const page = await settled(() => text('#page'), (value) => value !== pageBefore);
    return {
      ...environment,
      appRoot: document.querySelector('[data-fusor-version]') !== null,
      signal: {
        before,
        after: { text: clicked, pressed: button().getAttribute('aria-pressed') },
      },
      worker: await loaded('#worker'),
      fetch: {
        app: await loaded('#fetch'),
        status: await status('/greeting.txt'),
        missing: await status('/missing.js'),
      },
      history: { path: location.pathname, page, added: history.length - lengthBefore },
    };
  };

  window.addEventListener('load', async () => {
    const saved = sessionStorage.getItem(SAVED);
    if (saved === null) {
      const report = await exercise();
      sessionStorage.setItem(SAVED, JSON.stringify({ report, errors }));
      location.reload();
      return;
    }
    sessionStorage.removeItem(SAVED);
    const first = JSON.parse(saved);
    await hydrated();
    const reload = { path: location.pathname, page: text('#page') };
    const report = { ...first.report, reload, errors: [...first.errors, ...errors] };
    window.ipc.postMessage(JSON.stringify({ type: 'report', report }));
  });
})();
