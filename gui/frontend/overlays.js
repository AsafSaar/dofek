/* Dofek GUI — help/about overlay, settings, and plugin-manager wiring.
   Extracted verbatim from the second inline <script> in index.html. It
   must stay after the overlay markup it binds listeners to, so it keeps
   its original position rather than moving into app.js. */
document.getElementById('help-overlay').addEventListener('click',e=>{
  if(e.target.id==='help-overlay') e.target.classList.remove('visible');
});
document.getElementById('about-overlay').addEventListener('click',e=>{
  if(e.target.id==='about-overlay') e.target.classList.remove('visible');
});
document.querySelector('.logo').style.cursor='pointer';
document.querySelector('.logo').addEventListener('click',()=>{
  document.getElementById('about-overlay').classList.toggle('visible');
});
document.getElementById('open-manual-btn').addEventListener('click', async (e) => {
  e.stopPropagation();
  if (!invoke) return;
  try { await invoke('open_manual'); }
  catch(err) { console.error('Failed to open manual:', err); }
});
document.getElementById('telem-toggle-cb').addEventListener('change', async (e) => {
  if (!invoke) return;
  try {
    await invoke('set_telemetry_choice', { enabled: e.target.checked });
    const t=document.getElementById('toast');
    t.textContent=e.target.checked ? 'Telemetry enabled' : 'Telemetry disabled';
    t.classList.add('show');
    setTimeout(()=>t.classList.remove('show'),2000);
  } catch(err) { console.error('Failed to save telemetry choice:', err); }
});

async function patchSettings(patch) {
  if (!invoke) return;
  try {
    const cur = await invoke('get_settings');
    await invoke('save_settings', { settings: Object.assign({}, cur, patch) });
  } catch(err) { console.error('Failed to save tray setting:', err); }
}
document.getElementById('tray-enable-cb').addEventListener('change', e => patchSettings({ enable_tray: e.target.checked }));
document.getElementById('tray-close-cb').addEventListener('change', e => patchSettings({ close_to_tray: e.target.checked }));
document.getElementById('tray-mode-select').addEventListener('change', e => {
  // Keep the legacy boolean roughly in sync so a downgrade still produces a
  // sensible UI: "text" and "chart+text" both imply text-on.
  patchSettings({
    tray_display_mode: e.target.value,
    tray_show_text: e.target.value !== 'chart',
  });
});
document.getElementById('update-startup-cb').addEventListener('change', e => patchSettings({ check_updates_on_startup: e.target.checked }));

/* Update check — manual button + opt-in startup probe.
   Notify-only: we display version + a clickable release link, never download. */
function showToast(msg, ms=2400) {
  const t = document.getElementById('toast');
  t.textContent = msg;
  t.classList.add('show');
  setTimeout(()=>t.classList.remove('show'), ms);
}

async function runUpdateCheck({silent=false} = {}) {
  if (!invoke) return null;
  const statusEl = document.getElementById('update-status');
  if (statusEl && !silent) statusEl.textContent = 'Checking…';
  try {
    const info = await invoke('check_for_update');
    if (info.is_newer) {
      // Topbar pill: turn into a clickable "v… ↑" link to the release URL.
      const top = document.getElementById('app-version');
      if (top) {
        top.textContent = `v${info.current} → v${info.latest} ↑`;
        top.style.color = 'var(--mem)';
        top.style.cursor = 'pointer';
        top.title = `Dofek v${info.latest} is available — click to view release`;
        top.onclick = () => invoke('open_url', { url: info.url }).catch(()=>{});
      }
      if (statusEl) {
        // SAFE: info.latest is escaped. It comes from the update endpoint's
        // JSON, so it is remote-controlled, not a literal.
        statusEl.innerHTML = `<span style="color:var(--mem);font-weight:600;">v${esc(info.latest)} available</span>`;
      }
      // How to get it for *this* install. `brew upgrade` for a Homebrew copy
      // rather than "download the dmg", which would leave two installs
      // fighting over one config dir. Locally generated, but textContent
      // anyway — the hint is data on the same payload as remote fields.
      const hintEl = document.getElementById('update-hint');
      if (hintEl) {
        hintEl.textContent = info.hint || '';
        hintEl.hidden = !info.hint;
      }
      if (!silent) showToast(`Dofek v${info.latest} is available`);
      else showToast(`Update available: Dofek v${info.latest}`, 4000);
    } else {
      if (statusEl) statusEl.textContent = `Up to date (v${info.current})`;
      if (!silent) showToast(`You're on the latest release (v${info.current})`);
    }
    return info;
  } catch (e) {
    if (statusEl && !silent) statusEl.textContent = 'Update check failed';
    if (!silent) showToast(`Update check failed: ${e}`, 3500);
    console.warn('Update check failed:', e);
    return null;
  }
}

document.getElementById('check-update-btn').addEventListener('click', e => {
  e.stopPropagation();
  runUpdateCheck();
});

/* ---------- Plugin manager (settings → Plugins section) ---------- */
/* The Rust side owns install/uninstall — this UI just calls plugins_*
   IPC commands and refreshes the list. Hot-reload of the running plugin
   manager isn't wired yet, so we tell the user to restart for changes
   to take effect after add/remove. */

async function refreshPluginsList() {
  if (!invoke) return;
  const empty = document.getElementById('plugins-empty');
  const rows = document.getElementById('plugins-rows');
  if (!rows || !empty) return;
  try {
    const list = await invoke('plugins_list');
    // SAFE: clears the container; empty string literal.
    rows.innerHTML = '';
    if (!list || list.length === 0) {
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';
    for (const p of list) {
      const row = document.createElement('div');
      row.className = 'plugin-row';
      const meta = document.createElement('div');
      meta.className = 'plugin-meta';
      const titleLine = document.createElement('div');
      titleLine.className = 'plugin-title-line';
      const name = document.createElement('span');
      name.className = 'plugin-name';
      name.textContent = p.name;
      const ver = document.createElement('span');
      ver.className = 'plugin-version';
      ver.textContent = p.version ? `v${p.version}` : '';
      titleLine.appendChild(name);
      titleLine.appendChild(ver);
      const desc = document.createElement('div');
      desc.className = 'plugin-desc';
      desc.textContent = p.description || '(no description)';
      meta.appendChild(titleLine);
      meta.appendChild(desc);

      const ctrl = document.createElement('div');
      ctrl.className = 'plugin-ctrl';
      const toggle = document.createElement('label');
      toggle.className = 'toggle-switch';
      toggle.title = p.enabled ? 'Disable plugin' : 'Enable plugin';
      const cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.checked = !!p.enabled;
      cb.addEventListener('change', async () => {
        try {
          await invoke('plugins_set_enabled', { name: p.name, enabled: cb.checked });
          showToast(`${cb.checked ? 'Enabled' : 'Disabled'} ${p.name}`);
        } catch (e) {
          cb.checked = !cb.checked;
          showToast(`Failed: ${e}`, 3500);
        }
      });
      const slider = document.createElement('span');
      slider.className = 'toggle-slider';
      toggle.appendChild(cb);
      toggle.appendChild(slider);

      const removeBtn = document.createElement('button');
      removeBtn.className = 'plugin-remove';
      removeBtn.title = 'Uninstall plugin';
      removeBtn.textContent = '✕';
      removeBtn.addEventListener('click', async () => {
        if (!confirm(`Uninstall ${p.name}? This deletes the binary from Dofek's plugin directory.`)) return;
        try {
          await invoke('plugins_remove', { name: p.name });
          showToast(`Uninstalled ${p.name}`);
          refreshPluginsList();
        } catch (e) {
          showToast(`Remove failed: ${e}`, 3500);
        }
      });

      ctrl.appendChild(toggle);
      ctrl.appendChild(removeBtn);
      row.appendChild(meta);
      row.appendChild(ctrl);
      rows.appendChild(row);
    }
  } catch (e) {
    console.warn('plugins_list failed:', e);
  }
}

document.getElementById('add-plugin-btn').addEventListener('click', async (e) => {
  e.stopPropagation();
  if (!invoke) return;
  let path;
  try {
    path = await invoke('plugins_pick_file');
  } catch (err) {
    showToast(`File picker failed: ${err}`, 3500);
    return;
  }
  if (!path) return; // user cancelled

  if (!await confirmPluginInstall(path)) return;

  try {
    const installed = await invoke('plugins_add', { path, args: [] });
    showToast(`Installed ${installed.name}`, 3500);
    refreshPluginsList();
  } catch (err) {
    showToast(`Install failed: ${err}`, 4500);
  }
});

/* Confirm before installing. The Rust side independently requires that `path`
   came from the native picker, so this modal is about informed consent, not
   about being the security boundary. */
function confirmPluginInstall(path) {
  return new Promise(resolve => {
    const modal = document.getElementById('plugin-modal');
    const msg = document.getElementById('plugin-modal-msg');
    const yes = document.getElementById('plugin-modal-yes');
    const no = document.getElementById('plugin-modal-no');

    // textContent, not innerHTML: `path` is a filesystem path chosen by the
    // user and can contain anything a file name can.
    msg.textContent = '';
    const label = document.createElement('div');
    label.textContent = 'Install this plugin?';
    const pathEl = document.createElement('div');
    pathEl.style.cssText = 'color:var(--dim);word-break:break-all;margin-top:4px;';
    pathEl.textContent = path;
    msg.appendChild(label);
    msg.appendChild(pathEl);

    const finish = (ok) => {
      modal.classList.remove('visible');
      yes.removeEventListener('click', onYes);
      no.removeEventListener('click', onNo);
      resolve(ok);
    };
    const onYes = () => finish(true);
    const onNo = () => finish(false);
    yes.addEventListener('click', onYes);
    no.addEventListener('click', onNo);
    modal.classList.add('visible');
  });
}

document.getElementById('open-plugins-docs').addEventListener('click', (e) => {
  e.preventDefault();
  e.stopPropagation();
  if (invoke) invoke('open_url', { url: 'https://dofek.dev/plugins/' }).catch(()=>{});
});


/* Opt-in startup probe — fires once on boot if the user enabled it. */
if (invoke) {
  invoke('get_settings').then(s => {
    if (s && s.check_updates_on_startup) {
      // Defer slightly so the dashboard paints first.
      setTimeout(() => runUpdateCheck({silent:true}), 1500);
    }
  }).catch(()=>{});
}
