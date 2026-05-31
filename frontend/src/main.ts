import "./styles.css";

type ChannelState = {
  id: number;
  name: string;
  mute: boolean;
  fader_db: number;
  pan: number;
};

type MixerState = {
  device_profile: string;
  sample_rate: number;
  buffer_size: number;
  channels: ChannelState[];
  master: {
    mute: boolean;
    fader_db: number;
  };
};

type MeterState = {
  channels_peak_db: number[];
  master_peak_db: [number, number];
};

const API_BASE = "http://127.0.0.1:3798/api";
const app = document.querySelector<HTMLDivElement>("#app");

let mixer: MixerState | null = null;
let meters: MeterState | null = null;

async function loadMixer() {
  const response = await fetch(`${API_BASE}/mixer`);
  mixer = await response.json();
  render();
}

async function loadMeters() {
  const response = await fetch(`${API_BASE}/meters`);
  meters = await response.json();
  renderMeters();
}

async function patchChannel(channelId: number, patch: Partial<ChannelState>) {
  const response = await fetch(`${API_BASE}/channels/${channelId}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(patch),
  });

  mixer = await response.json();
  render();
}

async function saveScene() {
  await fetch(`${API_BASE}/scenes/default`, { method: "POST" });
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
        <button id="save-scene" class="primary">Save JSON</button>
      </header>

      <section class="mixer" aria-label="Mixer channels">
        ${mixer.channels.map(renderChannel).join("")}
        ${renderMaster()}
      </section>
    </main>
  `;

  document.querySelector("#save-scene")?.addEventListener("click", saveScene);

  for (const channel of mixer.channels) {
    document
      .querySelector<HTMLInputElement>(`#fader-${channel.id}`)
      ?.addEventListener("input", (event) => {
        const target = event.target as HTMLInputElement;
        patchChannel(channel.id, { fader_db: Number(target.value) });
      });

    document
      .querySelector<HTMLInputElement>(`#pan-${channel.id}`)
      ?.addEventListener("input", (event) => {
        const target = event.target as HTMLInputElement;
        patchChannel(channel.id, { pan: Number(target.value) });
      });

    document
      .querySelector<HTMLButtonElement>(`#mute-${channel.id}`)
      ?.addEventListener("click", () => {
        patchChannel(channel.id, { mute: !channel.mute });
      });
  }

  renderMeters();
}

function renderChannel(channel: ChannelState) {
  return `
    <article class="strip">
      <div class="strip-name">${channel.name}</div>
      <div class="meter" id="meter-${channel.id}">
        <div class="meter-fill"></div>
      </div>
      <label>
        Fader
        <input id="fader-${channel.id}" class="fader" type="range" min="-60" max="10" step="0.5" value="${channel.fader_db}" orient="vertical" />
      </label>
      <output>${channel.fader_db.toFixed(1)} dB</output>
      <label>
        Pan
        <input id="pan-${channel.id}" type="range" min="-1" max="1" step="0.01" value="${channel.pan}" />
      </label>
      <button id="mute-${channel.id}" class="${channel.mute ? "mute active" : "mute"}">Mute</button>
    </article>
  `;
}

function renderMaster() {
  return `
    <article class="strip master">
      <div class="strip-name">Main LR</div>
      <div class="meter stereo" id="meter-master-l"><div class="meter-fill"></div></div>
      <div class="meter stereo" id="meter-master-r"><div class="meter-fill"></div></div>
      <div class="master-label">Stereo Out</div>
    </article>
  `;
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

function dbToPercent(db: number) {
  return Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
}

loadMixer();
setInterval(loadMeters, 100);

