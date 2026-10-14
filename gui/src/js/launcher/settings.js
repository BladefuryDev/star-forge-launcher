import { invoke } from './bridge.js';
import { object, boolean, string, choice } from './contracts.js';
import { brand, required, run, errorMessage, notice } from './ui.js';
import { factions } from './preferences.js';

/** @param {import('./preferences.js').PreferenceStore} preferences
 * @param {ReturnType<typeof import('./state.js').createLauncherStore>} store */
export async function bindSettings(preferences, store) {
  let tab = 'launcher';
  let compat = { supported: false, proton: /** @type {string[]} */ ([]) };
  const area = required('settings-content');
  /** @param {string} title @param {string} note @param {string} id @param {string} input */
  const row = (title, note, id, input) =>
    `<div class="setting-row"><label for="${id}">${title}<small>${note}</small></label>${input}</div>`;
  const render = () => {
    area.innerHTML =
      tab === 'launcher'
        ? `
      <section class="setting-group"><h2>Window behavior</h2>${row('After starting the game', 'Choose where the launcher stays while you play.', 'launch-behavior', '<select id="launch-behavior" data-preference="launchBehavior"><option value="tray">Minimize to tray</option><option value="keepOpen">Keep launcher open</option><option value="exit">Exit launcher</option></select>')}
      ${row('When closing the launcher', 'Choose what happens when you click X.', 'close-behavior', '<select id="close-behavior" data-preference="closeBehavior"><option value="tray">Minimize to tray</option><option value="exit">Exit launcher</option></select>')}</section>
      <section class="setting-group"><h2>Appearance</h2><span>Faction color</span><div class="faction-options">${factions.map((name) => `<label class="faction-option"><input type="radio" name="faction-theme" data-preference="factionTheme" value="${name}"><span>${brand(name[0].toUpperCase() + name.slice(1))}</span></label>`).join('')}</div></section>`
        : `
      <section class="setting-group"><h2>Installation</h2>
      ${row('Game folder', 'Leave empty to find Star Conflict through Steam.', 'game-directory', '<input id="game-directory" data-preference="gameDirectory" type="text" spellcheck="false" placeholder="Found through Steam">')}
      <p class="settings-path" id="detected-game"></p></section>
      <section class="setting-group" ${compat.supported ? '' : 'hidden'}><h2>Compatibility</h2>
      ${row('Proton version', 'Automatic uses the Proton that last ran the game from Steam, with the same prefix and saved settings.', 'proton-path', '<select id="proton-path" data-preference="protonPath"><option value="">Automatic</option></select>')}
      <p class="settings-path" id="detected-proton"></p></section>
      <section class="setting-group"><h2>Folders and logs</h2><div class="setting-buttons"><button class="button" data-command="view_game_install">Open game folder</button><button class="button" data-command="open_profile_folder">Open profiles folder</button><button class="button" data-command="view_launch_log">Open last launch log</button></div></section>`;
    if (tab === 'game') {
      const select = area.querySelector('#proton-path');
      if (select instanceof HTMLSelectElement)
        for (const path of compat.proton)
          select.add(new Option(path.split(/[\\/]/).filter(Boolean).pop() || path, path));
    }
    sync();
  };
  const sync = () => {
    const values = preferences.get();
    document.documentElement.dataset.faction = values.factionTheme;
    area.querySelectorAll('[data-preference]').forEach((input) => {
      if (!(input instanceof HTMLInputElement || input instanceof HTMLSelectElement)) return;
      if (input === document.activeElement && input.type === 'text') return;
      const entry = Object.entries(values).find(([key]) => key === input.dataset.preference);
      if (!entry) return;
      if (input instanceof HTMLInputElement && input.type === 'radio')
        input.checked = input.value === entry[1];
      else input.value = String(entry[1]);
    });
    const state = store.getSnapshot();
    const game = area.querySelector('#detected-game');
    if (game)
      game.textContent = state?.gameDirectory
        ? `Using ${state.gameDirectory}`
        : state?.phase === 'checking'
          ? 'Looking for the game…'
          : 'Star Conflict was not found.';
    const proton = area.querySelector('#detected-proton');
    if (proton) proton.textContent = state?.proton ? `Using ${state.proton}` : '';
  };
  preferences.subscribe(sync);
  store.subscribe(sync);
  area.addEventListener('change', (event) => {
    const input = event.target;
    if (!(input instanceof HTMLInputElement || input instanceof HTMLSelectElement)) return;
    const key = input.dataset.preference;
    if (key === 'factionTheme') run(() => preferences.update(key, choice(input.value, factions)));
    else if (key === 'launchBehavior')
      run(() => preferences.update(key, choice(input.value, ['tray', 'keepOpen', 'exit'])));
    else if (key === 'closeBehavior')
      run(() => preferences.update(key, choice(input.value, ['tray', 'exit'])));
    else if (key === 'gameDirectory' || key === 'protonPath')
      run(() => preferences.update(key, input.value.trim()));
  });
  document.addEventListener('click', (event) => {
    if (!(event.target instanceof Element)) return;
    const button = event.target.closest('button');
    if (!button) return;
    if (button.dataset.settingsTab) {
      tab = button.dataset.settingsTab;
      document
        .querySelectorAll('[data-settings-tab]')
        .forEach((item) => item.removeAttribute('aria-current'));
      button.setAttribute('aria-current', 'page');
      render();
    }
    if (button.id === 'open-settings') run(() => preferences.reload());
    if (button.dataset.command && button.closest('#settings-content'))
      run(() => invoke(button.dataset.command || ''));
  });
  render();
  try {
    const value = object(await invoke('detect_compat_tools'));
    compat = {
      supported: boolean(value.supported),
      proton: Array.isArray(value.proton) ? value.proton.map(string) : [],
    };
    required('launcher-version').textContent = `STAR FORGE Launcher ${string(await invoke('app_version'))}`;
    render();
  } catch (error) {
    notice(`Could not load launcher settings: ${errorMessage(error)}`, 'error');
  }
}
