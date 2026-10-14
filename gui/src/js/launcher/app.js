import { mountLayout } from './layout.js';
import { launcherBridge, invoke, listen } from './bridge.js';
import { createLauncherStore } from './state.js';
import { loadPreferences } from './preferences.js';
import { bindShell } from './shell.js';
import { bindSettings } from './settings.js';
import { bindProfiles } from './profiles.js';
import { bindArtwork } from './artwork.js';
import { startupStep, showStartupFailure } from './startup.js';
import { control, required, notice, run } from './ui.js';

mountLayout();

async function start() {
  const preferences = await startupStep(loadPreferences(), 'Loading launcher settings');
  document.documentElement.dataset.faction = preferences.get().factionTheme;
  const store = createLauncherStore(launcherBridge);
  const profiles = bindProfiles(preferences, () => render());
  /** @type {Record<import('./state.js').LauncherPhase, string>} */
  const labels = {
    checking: 'CHECKING',
    ready: 'PLAY',
    launching: 'STARTING',
    running: 'RUNNING',
    failed: 'RETRY',
  };
  /** @type {Record<import('./state.js').LauncherPhase, string>} */
  const statuses = {
    checking: 'Looking for Star Conflict…',
    ready: 'Ready to launch',
    launching: 'Starting the game…',
    running: 'Game is running',
    failed: 'Action needed',
  };
  let lastReason = '';
  const render = () => {
    const state = store.getSnapshot();
    if (!state) return;
    const button = control('play-button', HTMLButtonElement);
    required('play-label').textContent = labels[state.phase];
    button.disabled =
      state.busy ||
      store.isRequestPending() ||
      !state.primaryAction ||
      (state.primaryAction === 'launch' && !profiles.ready());
    const status = required('launch-status');
    status.textContent =
      state.phase === 'ready' && !profiles.ready()
        ? 'Choose or add a server profile'
        : statuses[state.phase];
    status.dataset.ready = String(state.phase === 'ready' && profiles.ready());
    if (state.blockingReason && state.blockingReason !== lastReason)
      notice(state.blockingReason, 'error');
    lastReason = state.blockingReason || '';
    // Profiles can't change under a running game.
    const locked = state.phase === 'launching' || state.phase === 'running';
    const select = control('profile-select', HTMLSelectElement);
    select.disabled = locked || !select.options.length || select.options[0].value === '';
    control('add-profile', HTMLButtonElement).disabled = locked;
    control('edit-profile', HTMLButtonElement).disabled = locked || !profiles.current();
  };
  bindShell(() => render());
  store.subscribe(render);
  required('play-button').addEventListener('click', () => {
    const action = store.getSnapshot()?.primaryAction;
    if (action) run(() => store.performAction(action));
  });
  await startupStep(
    listen('notification', (payload) => {
      if (!Array.isArray(payload) || typeof payload[0] !== 'string')
        throw new Error('Invalid notification event');
      notice(payload[0], payload[1] === 2 ? 'error' : payload[1] === 1 ? 'success' : 'info');
    }),
    'Registering launcher notifications',
  );
  window.addEventListener('beforeunload', () => store.dispose());
  try {
    await startupStep(store.start(), 'Loading launcher state');
    await startupStep(profiles.refresh(), 'Loading server profiles');
    await startupStep(invoke('main_ready'), 'Showing launcher');
    run(() => bindSettings(preferences, store));
    run(bindArtwork);
    run(() => invoke('initialize_launcher'));
  } catch (error) {
    store.dispose();
    throw error;
  }
}

start().catch((error) => run(() => showStartupFailure(error)));
