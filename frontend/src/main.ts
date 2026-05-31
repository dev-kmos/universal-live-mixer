import "./styles.css";

type ChannelState = {
  id: number;
  name: string;
  mute: boolean;
  fader_db: number;
  pan: number;
};

type MasterState = {
  mute: boolean;
  fader_db: number;
};

type MixerState = {
  device_profile: string;
  sample_rate: number;
  buffer_size: number;
  channels: ChannelState[];
  master: MasterState;
};

type MeterState = {
  channels_peak_db: number[];
  master_peak_db: [number, number];
};

type SceneEntry = {
  id: string;
  name: string;
  file: string;
};

type SceneList = {
  version: number;
  current_scene_id: string | null;
  scenes: SceneEntry[];
};

type SceneLoadResult = {
  scenes: SceneList;
  mixer: MixerState;
};

type ControlKind = "fader" | "pan";
type ControlTarget = { type: "channel"; channelId: number } | { type: "master" };

const API_BASE = "http://127.0.0.1:3798/api";
const FADER_MIN = -60;
const FADER_MAX = 10;
const PAN_MIN = -1;
const PAN_MAX = 1;
const FADER_SCALE = [10, 5, 0, -5, -10, -20, -30, -40, -50, -60];
const app = document.querySelector<HTMLDivElement>("#app");

let mixer: MixerState | null = null;
let meters: MeterState | null = null;
let sceneList: SceneList | null = null;
let mixerRevision = 0;
let masterPatchSeq = 0;
const channelPatchSeq = new Map<number, number>();
let activePointer: { control: HTMLElement; target: ControlTarget; kind: ControlKind } | null = null;

async function initialize() {
  const [mixerResponse, scenesResponse] = await Promise.all([
    fetch(`${API_BASE}/mixer`),
    fetch(`${API_BASE}/scenes`),
  ]);

  mixer = await mixerResponse.json();
  sceneList = await scenesResponse.json();
  render();
}

async function loadMeters() {
  const response = await fetch(`${API_BASE}/meters`);
  meters = await response.json();
  renderMeters();
}

async function patchChannel(channelId: number, patch: Partial<ChannelState>) {
  const requestRevision = mixerRevision;
  const requestSeq = nextChannelPatchSeq(channelId);
  const response = await fetch(`${API_BASE}/channels/${channelId}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(patch),
  });

  if (requestRevision !== mixerRevision || requestSeq !== channelPatchSeq.get(channelId)) {
    return;
  }

  const serverMixer: MixerState = await response.json();
  const serverChannel = serverMixer.channels.find((item) => item.id === channelId);
  const localChannel = findChannel(channelId);
  if (!serverChannel || !localChannel) return;

  if (patch.name !== undefined) localChannel.name = serverChannel.name;
  if (patch.mute !== undefined) localChannel.mute = serverChannel.mute;
  if (patch.fader_db !== undefined) localChannel.fader_db = serverChannel.fader_db;
  if (patch.pan !== undefined) localChannel.pan = serverChannel.pan;
  updateChannelDom(localChannel);
}

async function patchMaster(patch: Partial<MasterState>) {
  const requestRevision = mixerRevision;
  const requestSeq = ++masterPatchSeq;
  const response = await fetch(`${API_BASE}/master`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(patch),
  });

  if (requestRevision !== mixerRevision || requestSeq !== masterPatchSeq || !mixer) {
    return;
  }

  const serverMixer: MixerState = await response.json();
  if (patch.fader_db !== undefined) mixer.master.fader_db = serverMixer.master.fader_db;
  updateMasterDom(mixer.master);
}

async function loadScenes() {
  const response = await fetch(`${API_BASE}/scenes`);
  sceneList = await response.json();
  render();
}

async function newScene() {
  const nextNumber = (sceneList?.scenes.length ?? 0) + 1;
  const response = await fetch(`${API_BASE}/scenes`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name: `Scene ${nextNumber}` }),
  });
  sceneList = await response.json();
  render();
}

async function saveCurrentScene() {
  if (!sceneList?.current_scene_id) {
    await newScene();
    return;
  }

  const response = await fetch(`${API_BASE}/scenes/${sceneList.current_scene_id}/save`, {
    method: "POST",
  });
  sceneList = await response.json();
  renderSceneManager();
}

async function loadScene(sceneId: string) {
  const response = await fetch(`${API_BASE}/scenes/${sceneId}/load`, { method: "POST" });
  const result: SceneLoadResult = await response.json();
  applySceneLoadResult(result);
}

async function loadNextScene() {
  const response = await fetch(`${API_BASE}/scenes/next`, { method: "POST" });
  const result: SceneLoadResult = await response.json();
  applySceneLoadResult(result);
}

async function loadPreviousScene() {
  const response = await fetch(`${API_BASE}/scenes/previous`, { method: "POST" });
  const result: SceneLoadResult = await response.json();
  applySceneLoadResult(result);
}

async function reloadCurrentScene() {
  if (!sceneList?.current_scene_id) return;

  const response = await fetch(`${API_BASE}/scenes/current/reload`, { method: "POST" });
  if (!response.ok) return;

  const result: SceneLoadResult = await response.json();
  applySceneLoadResult(result);
}

async function renameScene(sceneId: string, name: string) {
  const response = await fetch(`${API_BASE}/scenes/${sceneId}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
  sceneList = await response.json();
  renderSceneManager();
}

async function moveScene(sceneId: string, direction: "up" | "down") {
  const response = await fetch(`${API_BASE}/scenes/${sceneId}/move`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ direction }),
  });
  sceneList = await response.json();
  renderSceneManager();
}

function applySceneLoadResult(result: SceneLoadResult) {
  mixerRevision += 1;
  masterPatchSeq += 1;
  channelPatchSeq.clear();
  mixer = result.mixer;
  sceneList = result.scenes;
  render();
}

function render() {
  if (!app || !mixer) {
    return;
  }

  app.innerHTML = `
    <main class="shell">
      <header class="topbar">
        <div>
          <h1>Universal Live Mixer</h1>
          <p>${mixer.device_profile} · ${mixer.sample_rate} Hz · ${mixer.buffer_size} frames</p>
        </div>
      </header>

      <section id="scene-panel">
        ${renderSceneManagerHtml()}
      </section>

      <section class="mixer-surface" aria-label="Mixer">
        <div class="input-mixer" aria-label="Input channels">
          ${mixer.channels.map(renderChannel).join("")}
        </div>
        ${renderMaster(mixer.master)}
      </section>
    </main>
  `;

  bindSceneControls();
  bindControls();
  renderMeters();
}

function renderSceneManager() {
  const panel = document.querySelector<HTMLElement>("#scene-panel");
  if (!panel) return;

  panel.innerHTML = renderSceneManagerHtml();
  bindSceneControls();
}

function renderSceneManagerHtml() {
  const scenes = sceneList?.scenes ?? [];
  const currentId = sceneList?.current_scene_id ?? null;
  const reloadDisabled = currentId ? "" : "disabled";

  return `
    <section class="scene-manager" aria-label="Scenes">
      <div class="scene-header">
        <h2>Scenes</h2>
        <div class="scene-actions">
          <button type="button" id="previous-scene" class="secondary">Previous Scene</button>
          <button type="button" id="next-scene" class="secondary">Next Scene</button>
          <button type="button" id="reload-scene" class="secondary" ${reloadDisabled}>Reload Scene</button>
          <button type="button" id="save-scene" class="primary">Save Scene</button>
          <button type="button" id="new-scene" class="secondary">New Scene</button>
        </div>
      </div>
      <ol class="scene-list">
        ${
          scenes.length === 0
            ? '<li class="scene-empty">No scenes yet</li>'
            : scenes.map((scene, index) => renderSceneItem(scene, index, currentId)).join("")
        }
      </ol>
    </section>
  `;
}

function renderSceneItem(scene: SceneEntry, index: number, currentId: string | null) {
  const active = scene.id === currentId;
  return `
    <li class="${active ? "scene-item active" : "scene-item"}" data-scene-id="${scene.id}">
      <button type="button" class="scene-load" data-scene-load="${scene.id}">
        <span class="scene-number">${String(index + 1).padStart(2, "0")}.</span>
      </button>
      <input class="scene-name-input" data-scene-rename="${scene.id}" value="${escapeHtml(scene.name)}" aria-label="Scene name" />
      <button type="button" class="scene-move" data-scene-move="${scene.id}" data-direction="up" aria-label="Move scene up">↑</button>
      <button type="button" class="scene-move" data-scene-move="${scene.id}" data-direction="down" aria-label="Move scene down">↓</button>
    </li>
  `;
}

function bindSceneControls() {
  document.querySelector("#previous-scene")?.addEventListener("click", () => void loadPreviousScene());
  document.querySelector("#next-scene")?.addEventListener("click", () => void loadNextScene());
  document.querySelector("#reload-scene")?.addEventListener("click", () => void reloadCurrentScene());
  document.querySelector("#save-scene")?.addEventListener("click", () => void saveCurrentScene());
  document.querySelector("#new-scene")?.addEventListener("click", () => void newScene());

  document.querySelectorAll<HTMLElement>("[data-scene-load]").forEach((button) => {
    button.addEventListener("click", () => {
      const sceneId = button.dataset.sceneLoad;
      if (sceneId) void loadScene(sceneId);
    });
  });

  document.querySelectorAll<HTMLElement>("[data-scene-move]").forEach((button) => {
    button.addEventListener("click", () => {
      const sceneId = button.dataset.sceneMove;
      const direction = button.dataset.direction === "up" ? "up" : "down";
      if (sceneId) void moveScene(sceneId, direction);
    });
  });

  document.querySelectorAll<HTMLInputElement>("[data-scene-rename]").forEach((input) => {
    const sceneId = input.dataset.sceneRename;
    const commit = () => {
      if (!sceneId) return;
      const current = sceneList?.scenes.find((scene) => scene.id === sceneId);
      const name = input.value.trim();
      if (!name || current?.name === name) {
        input.value = current?.name ?? input.value;
        return;
      }
      void renameScene(sceneId, name);
    };

    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        commit();
        input.blur();
      }
    });
    input.addEventListener("blur", commit);
  });
}

function renderChannel(channel: ChannelState) {
  return `
    <article class="strip" data-channel-strip="${channel.id}">
      <div class="strip-name">${channel.name}</div>

      <div class="strip-main">
        <div class="fader-zone">
          <div class="fader-scale" aria-hidden="true">
            ${renderFaderScale()}
          </div>
          ${renderSlider("fader", { type: "channel", channelId: channel.id }, channel.fader_db, FADER_MIN, FADER_MAX, `${channel.name} fader`)}
        </div>

        <div class="meter channel-meter" id="meter-${channel.id}" aria-label="${channel.name} meter">
          <div class="meter-fill"></div>
        </div>
      </div>

      <output id="fader-value-${channel.id}" class="fader-value">${formatDb(channel.fader_db)}</output>
      <input id="fader-input-${channel.id}" class="number-input" type="number" min="${FADER_MIN}" max="${FADER_MAX}" step="0.5" value="${channel.fader_db.toFixed(1)}" aria-label="${channel.name} fader value dB" />

      <div class="pan-row">
        <span class="pan-label">L</span>
        ${renderSlider("pan", { type: "channel", channelId: channel.id }, channel.pan, PAN_MIN, PAN_MAX, `${channel.name} pan`)}
        <span class="pan-label">R</span>
      </div>
      <output id="pan-value-${channel.id}" class="pan-value">${formatPan(channel.pan)}</output>
      <input id="pan-input-${channel.id}" class="number-input" type="number" min="${PAN_MIN}" max="${PAN_MAX}" step="0.01" value="${channel.pan.toFixed(2)}" aria-label="${channel.name} pan value" />

      <button id="mute-${channel.id}" type="button" aria-pressed="${channel.mute}" class="${channel.mute ? "mute active" : "mute"}">
        Mute
      </button>
    </article>
  `;
}

function renderMaster(master: MasterState) {
  return `
    <aside class="strip master" aria-label="Main LR master section">
      <div class="strip-name">Main LR</div>

      <div class="master-main">
        <div class="fader-zone">
          <div class="fader-scale" aria-hidden="true">
            ${renderFaderScale()}
          </div>
          ${renderSlider("fader", { type: "master" }, master.fader_db, FADER_MIN, FADER_MAX, "Main LR fader")}
        </div>

        <div class="master-meters">
          <div class="meter stereo" id="meter-master-l" aria-label="Main left meter">
            <div class="meter-fill"></div>
            <span>L</span>
          </div>
          <div class="meter stereo" id="meter-master-r" aria-label="Main right meter">
            <div class="meter-fill"></div>
            <span>R</span>
          </div>
        </div>
      </div>

      <output id="master-fader-value" class="fader-value">${formatDb(master.fader_db)}</output>
      <input id="master-fader-input" class="number-input" type="number" min="${FADER_MIN}" max="${FADER_MAX}" step="0.5" value="${master.fader_db.toFixed(1)}" aria-label="Main LR fader value dB" />
      <div class="master-label">Stereo Out</div>
    </aside>
  `;
}

function renderSlider(
  kind: ControlKind,
  target: ControlTarget,
  value: number,
  min: number,
  max: number,
  label: string,
) {
  const percent = valueToPercent(value, min, max);
  const orientation = kind === "fader" ? "vertical" : "horizontal";
  const targetKey = targetKeyFor(target);
  const channelAttr = target.type === "channel" ? `data-channel-id="${target.channelId}"` : "";

  return `
    <div
      id="${kind}-${targetKey}"
      class="${kind}-control slider-control"
      data-control="${kind}"
      data-target="${target.type}"
      ${channelAttr}
      role="slider"
      tabindex="0"
      aria-label="${label}"
      aria-orientation="${orientation}"
      aria-valuemin="${min}"
      aria-valuemax="${max}"
      aria-valuenow="${value}"
    >
      <div class="slider-track">
        <div class="slider-fill" style="${kind === "fader" ? "height" : "width"}: ${percent}%"></div>
        <div class="slider-thumb" style="${kind === "fader" ? "bottom" : "left"}: ${percent}%"></div>
        ${kind === "fader" ? '<div class="unity-line"></div>' : '<div class="pan-center-line"></div>'}
      </div>
    </div>
  `;
}

function renderFaderScale() {
  const marks = FADER_SCALE.map((value) => {
    const percent = valueToPercent(value, FADER_MIN, FADER_MAX);
    const label = value > 0 ? `+${value}` : String(value);
    return `<span class="${value === 0 ? "scale-mark unity" : "scale-mark"}" style="bottom: ${percent}%">${label}</span>`;
  });

  marks.push('<span class="scale-mark infinity" style="bottom: 0%">-∞</span>');
  return marks.join("");
}

function bindControls() {
  document.querySelectorAll<HTMLElement>("[data-control]").forEach((control) => {
    const target = controlTargetFromElement(control);
    const kind = control.dataset.control as ControlKind;

    control.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      control.setPointerCapture(event.pointerId);
      document.body.classList.add("dragging-control");
      activePointer = { control, target, kind };
      updateFromPointer(control, target, kind, event);
    });

    control.addEventListener("pointermove", (event) => {
      if (activePointer?.control !== control) return;
      event.preventDefault();
      updateFromPointer(control, target, kind, event);
    });

    control.addEventListener("pointerup", (event) => finishPointerDrag(control, event.pointerId));
    control.addEventListener("pointercancel", (event) => finishPointerDrag(control, event.pointerId));

    control.addEventListener("wheel", (event) => {
      event.preventDefault();
      const current = getControlCurrentValue(target, kind);
      if (current === null) return;

      const direction = event.deltaY > 0 ? -1 : 1;
      const step = kind === "fader" ? (event.shiftKey ? 0.1 : 0.5) : event.shiftKey ? 0.01 : 0.05;
      setControlValue(target, kind, current + direction * step);
    });

    control.addEventListener("dblclick", (event) => {
      event.preventDefault();
      setControlValue(target, kind, 0.0);
    });

    control.addEventListener("keydown", (event) => {
      const current = getControlCurrentValue(target, kind);
      if (current === null) return;

      if (kind === "fader" && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
        event.preventDefault();
        const step = event.shiftKey ? 0.1 : 0.5;
        setControlValue(target, kind, current + (event.key === "ArrowUp" ? step : -step));
      }

      if (kind === "pan" && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
        event.preventDefault();
        const step = event.shiftKey ? 0.01 : 0.05;
        setControlValue(target, kind, current + (event.key === "ArrowRight" ? step : -step));
      }
    });
  });

  mixer?.channels.forEach((channel) => {
    document.querySelector<HTMLButtonElement>(`#mute-${channel.id}`)?.addEventListener("click", () => {
      const currentChannel = findChannel(channel.id);
      if (currentChannel) setMute(channel.id, !currentChannel.mute);
    });

    bindNumberInput(`fader-input-${channel.id}`, { type: "channel", channelId: channel.id }, "fader");
    bindNumberInput(`pan-input-${channel.id}`, { type: "channel", channelId: channel.id }, "pan");
  });

  bindNumberInput("master-fader-input", { type: "master" }, "fader");
}

function bindNumberInput(id: string, target: ControlTarget, kind: ControlKind) {
  const input = document.querySelector<HTMLInputElement>(`#${id}`);
  if (!input) return;

  const commit = () => {
    const parsed = Number(input.value);
    if (!Number.isFinite(parsed)) {
      syncNumberInputs(target);
      return;
    }
    setControlValue(target, kind, parsed);
  };

  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      commit();
      input.blur();
    }
  });
  input.addEventListener("blur", commit);
}

function finishPointerDrag(control: HTMLElement, pointerId: number) {
  if (activePointer?.control === control) {
    activePointer = null;
    document.body.classList.remove("dragging-control");
  }

  if (control.hasPointerCapture(pointerId)) {
    control.releasePointerCapture(pointerId);
  }
}

function updateFromPointer(
  control: HTMLElement,
  target: ControlTarget,
  kind: ControlKind,
  event: PointerEvent,
) {
  const rect = control.getBoundingClientRect();
  const percent =
    kind === "fader"
      ? clamp((rect.bottom - event.clientY) / rect.height, 0, 1)
      : clamp((event.clientX - rect.left) / rect.width, 0, 1);
  const value =
    kind === "fader"
      ? roundTo(FADER_MIN + percent * (FADER_MAX - FADER_MIN), 0.5)
      : roundTo(PAN_MIN + percent * (PAN_MAX - PAN_MIN), 0.01);

  setControlValue(target, kind, value);
}

function setControlValue(target: ControlTarget, kind: ControlKind, rawValue: number) {
  if (target.type === "master") {
    if (kind !== "fader" || !mixer) return;

    const value = clamp(roundTo(rawValue, 0.5), FADER_MIN, FADER_MAX);
    if (value === mixer.master.fader_db) return;
    mixer.master.fader_db = value;
    updateMasterDom(mixer.master);
    void patchMaster({ fader_db: value });
    return;
  }

  const channel = findChannel(target.channelId);
  if (!channel) return;

  if (kind === "fader") {
    const value = clamp(roundTo(rawValue, 0.5), FADER_MIN, FADER_MAX);
    if (value === channel.fader_db) return;
    channel.fader_db = value;
    updateChannelDom(channel);
    void patchChannel(target.channelId, { fader_db: value });
    return;
  }

  const value = clamp(roundTo(rawValue, 0.01), PAN_MIN, PAN_MAX);
  if (value === channel.pan) return;
  channel.pan = value;
  updateChannelDom(channel);
  void patchChannel(target.channelId, { pan: value });
}

function setMute(channelId: number, mute: boolean) {
  const channel = findChannel(channelId);
  if (!channel) return;

  channel.mute = mute;
  updateChannelDom(channel);
  void patchChannel(channelId, { mute });
}

function updateChannelDom(channel: ChannelState) {
  updateSliderDom("fader", { type: "channel", channelId: channel.id }, channel.fader_db, FADER_MIN, FADER_MAX);
  updateSliderDom("pan", { type: "channel", channelId: channel.id }, channel.pan, PAN_MIN, PAN_MAX);

  const faderValue = document.querySelector<HTMLOutputElement>(`#fader-value-${channel.id}`);
  if (faderValue) faderValue.value = formatDb(channel.fader_db);

  const panValue = document.querySelector<HTMLOutputElement>(`#pan-value-${channel.id}`);
  if (panValue) panValue.value = formatPan(channel.pan);

  syncNumberInputs({ type: "channel", channelId: channel.id });

  const mute = document.querySelector<HTMLButtonElement>(`#mute-${channel.id}`);
  if (mute) {
    mute.classList.toggle("active", channel.mute);
    mute.setAttribute("aria-pressed", String(channel.mute));
  }
}

function updateMasterDom(master: MasterState) {
  updateSliderDom("fader", { type: "master" }, master.fader_db, FADER_MIN, FADER_MAX);

  const faderValue = document.querySelector<HTMLOutputElement>("#master-fader-value");
  if (faderValue) faderValue.value = formatDb(master.fader_db);

  syncNumberInputs({ type: "master" });
}

function syncNumberInputs(target: ControlTarget) {
  if (target.type === "master") {
    if (!mixer) return;
    syncInputValue("master-fader-input", mixer.master.fader_db.toFixed(1));
    return;
  }

  const channel = findChannel(target.channelId);
  if (!channel) return;
  syncInputValue(`fader-input-${channel.id}`, channel.fader_db.toFixed(1));
  syncInputValue(`pan-input-${channel.id}`, channel.pan.toFixed(2));
}

function syncInputValue(id: string, value: string) {
  const input = document.querySelector<HTMLInputElement>(`#${id}`);
  if (input && document.activeElement !== input) {
    input.value = value;
  }
}

function updateSliderDom(kind: ControlKind, target: ControlTarget, value: number, min: number, max: number) {
  const control = document.querySelector<HTMLElement>(`#${kind}-${targetKeyFor(target)}`);
  if (!control) return;

  const percent = valueToPercent(value, min, max);
  control.setAttribute("aria-valuenow", String(value));

  const fill = control.querySelector<HTMLElement>(".slider-fill");
  const thumb = control.querySelector<HTMLElement>(".slider-thumb");
  if (fill) fill.style[kind === "fader" ? "height" : "width"] = `${percent}%`;
  if (thumb) thumb.style[kind === "fader" ? "bottom" : "left"] = `${percent}%`;
}

function renderMeters() {
  if (!meters) {
    return;
  }

  meters.channels_peak_db.forEach((value, index) => {
    const fill = document.querySelector<HTMLElement>(`#meter-${index} .meter-fill`);
    if (fill) {
      fill.style.height = `${dbToPercent(value)}%`;
    }
  });

  const masterLeft = document.querySelector<HTMLElement>("#meter-master-l .meter-fill");
  const masterRight = document.querySelector<HTMLElement>("#meter-master-r .meter-fill");

  if (masterLeft) masterLeft.style.height = `${dbToPercent(meters.master_peak_db[0])}%`;
  if (masterRight) masterRight.style.height = `${dbToPercent(meters.master_peak_db[1])}%`;
}

function nextChannelPatchSeq(channelId: number) {
  const next = (channelPatchSeq.get(channelId) ?? 0) + 1;
  channelPatchSeq.set(channelId, next);
  return next;
}

function controlTargetFromElement(control: HTMLElement): ControlTarget {
  if (control.dataset.target === "master") {
    return { type: "master" };
  }

  return { type: "channel", channelId: Number(control.dataset.channelId) };
}

function targetKeyFor(target: ControlTarget) {
  return target.type === "master" ? "master" : String(target.channelId);
}

function getControlCurrentValue(target: ControlTarget, kind: ControlKind) {
  if (target.type === "master") {
    return kind === "fader" && mixer ? mixer.master.fader_db : null;
  }

  const channel = findChannel(target.channelId);
  if (!channel) return null;
  return kind === "fader" ? channel.fader_db : channel.pan;
}

function findChannel(channelId: number) {
  return mixer?.channels.find((channel) => channel.id === channelId);
}

function valueToPercent(value: number, min: number, max: number) {
  return ((clamp(value, min, max) - min) / (max - min)) * 100;
}

function dbToPercent(db: number) {
  return Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
}

function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

function roundTo(value: number, step: number) {
  return Math.round(value / step) * step;
}

function formatDb(value: number) {
  return `${value > 0 ? "+" : ""}${value.toFixed(1)} dB`;
}

function formatPan(value: number) {
  if (value === 0) return "C";
  return `${value < 0 ? "L" : "R"} ${Math.abs(value).toFixed(2)}`;
}

function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

initialize();
setInterval(loadMeters, 100);
