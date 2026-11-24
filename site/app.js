(() => {
  const RELEASE_BASE = "https://github.com/AungMyoKyaw/block-clock/releases/latest/download";
  const THEME_STORAGE_KEY = "block-clock-theme";
  const themeBtn = document.getElementById("theme-toggle");

  function effectiveTheme() {
    const attr = document.documentElement.getAttribute("data-theme");
    if (attr === "dark" || attr === "light") return attr;
    return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function readStoredTheme() {
    try {
      return window.localStorage.getItem(THEME_STORAGE_KEY);
    } catch {
      return null;
    }
  }

  function writeStoredTheme(theme) {
    try {
      window.localStorage.setItem(THEME_STORAGE_KEY, theme);
      return true;
    } catch {
      return false;
    }
  }

  function applyTheme(theme, persist) {
    if (theme === "dark" || theme === "light") {
      document.documentElement.setAttribute("data-theme", theme);
      if (persist) {
        writeStoredTheme(theme);
      }
    }
    if (themeBtn) {
      const active = effectiveTheme();
      themeBtn.textContent = active === "dark" ? "Light" : "Dark";
      themeBtn.setAttribute("aria-pressed", String(active === "dark"));
      themeBtn.setAttribute("aria-label", `Switch to ${active === "dark" ? "light" : "dark"} theme`);
    }
  }

  applyTheme(readStoredTheme(), false);

  if (themeBtn) {
    themeBtn.addEventListener("click", () => {
      const next = effectiveTheme() === "dark" ? "light" : "dark";
      applyTheme(next, true);
    });
  }

  const gridEl = document.getElementById("block-grid");
  if (!gridEl) return;

  const liveTimeEl = document.getElementById("live-time");
  const liveDateEl = document.getElementById("live-date");
  const hoverReadoutEl = document.getElementById("hover-readout");
  const statBlockEl = document.getElementById("stat-block");
  const statHmEl = document.getElementById("stat-hm");
  const statPctEl = document.getElementById("stat-pct");
  const progressFillEl = document.getElementById("progress-fill");
  const progressTrackEl = document.querySelector(".progress-track");
  const selEmptyEl = document.getElementById("selection-empty");
  const selDetailEl = document.getElementById("selection-detail");
  const selTimeEl = document.getElementById("sel-time");
  const selStateEl = document.getElementById("sel-state");
  const selBlockEl = document.getElementById("sel-block");
  const selOffsetEl = document.getElementById("sel-offset");
  const clearBtn = document.getElementById("clear-selection");
  const jumpBtn = document.getElementById("jump-now");
  const formatBtn = document.getElementById("format-toggle");
  const primaryDownloadEl = document.getElementById("primary-download");

  let hour12 = false;
  let selected = null;
  let lastMinuteKey = "";
  const cells = [];

  function pad2(n) {
    return n < 10 ? "0" + n : String(n);
  }

  function formatTime(hour, minute) {
    if (!hour12) return `${pad2(hour)}:${pad2(minute)}`;
    const period = hour < 12 ? "AM" : "PM";
    const h = hour % 12 === 0 ? 12 : hour % 12;
    return `${h}:${pad2(minute)} ${period}`;
  }

  function stateAt(now, cell) {
    if (cell.hour < now.hour) return "past-day";
    if (cell.hour > now.hour) return "future";
    if (cell.minute < now.minute) return "current-hour-past";
    if (cell.minute === now.minute) return "now";
    return "future";
  }

  function stateLabel(state) {
    if (state === "past-day") return "Earlier today";
    if (state === "current-hour-past") return "Earlier this hour";
    if (state === "now") return "Current minute";
    return "Upcoming";
  }

  function relativeOffset(now, cell) {
    const diff = (cell.hour * 60 + cell.minute) - (now.hour * 60 + now.minute);
    if (diff === 0) return "Active now";
    const abs = Math.abs(diff);
    const hours = Math.floor(abs / 60);
    const mins = abs % 60;
    let span;
    if (hours === 0) span = `${mins}m`;
    else if (mins === 0) span = `${hours}h`;
    else span = `${hours}h ${pad2(mins)}m`;
    return diff < 0 ? `${span} ago` : `in ${span}`;
  }

  function currentNow() {
    const d = new Date();
    return { hour: d.getHours(), minute: d.getMinutes(), date: d };
  }

  function detectPrimaryDownload() {
    if (!primaryDownloadEl) return;
    const ua = navigator.userAgent || "";
    if (/iPhone|iPad|iPod|Android/i.test(ua)) {
      primaryDownloadEl.textContent = "View desktop downloads";
      primaryDownloadEl.href = "#downloads";
    } else if (/Windows/i.test(ua)) {
      primaryDownloadEl.textContent = "Download for Windows";
      primaryDownloadEl.href = `${RELEASE_BASE}/block-clock-windows-x64.zip`;
    } else if (/Linux/i.test(ua)) {
      primaryDownloadEl.textContent = "Download for Linux";
      primaryDownloadEl.href = `${RELEASE_BASE}/block-clock-linux-x64.tar.gz`;
    } else if (/Macintosh|Mac OS X/i.test(ua)) {
      primaryDownloadEl.textContent = "Download for macOS";
      primaryDownloadEl.href = "#downloads";
    } else {
      primaryDownloadEl.textContent = "Download";
      primaryDownloadEl.href = "#downloads";
    }
  }

  function buildGrid() {
    const frag = document.createDocumentFragment();
    for (let hour = 0; hour < 24; hour++) {
      const row = document.createElement("div");
      row.className = hour > 0 && hour % 6 === 0 ? "hour-row hour-break" : "hour-row";

      const label = document.createElement("div");
      label.className = "hour-label";
      label.textContent = pad2(hour);
      row.appendChild(label);

      const track = document.createElement("div");
      track.className = "hour-track";

      for (let minute = 0; minute < 60; minute++) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "cell";
        const cell = { hour, minute };
        const blockNum = hour * 60 + minute + 1;
        btn.setAttribute("aria-label", `${pad2(hour)}:${pad2(minute)}, Block ${blockNum}`);
        btn.addEventListener("mouseenter", () => {
          hoverReadoutEl.textContent = `${formatTime(hour, minute)} · Block ${blockNum} / 1440`;
        });
        btn.addEventListener("mouseleave", () => {
          hoverReadoutEl.textContent = "";
        });
        btn.addEventListener("click", () => {
          selected = cell;
          render();
        });
        cells.push({ cell, btn });
        track.appendChild(btn);
      }

      row.appendChild(track);
      frag.appendChild(row);
    }
    gridEl.appendChild(frag);
  }

  function renderTimezone(d) {
    const tzEl = document.getElementById("tz-label");
    const dstEl = document.getElementById("dst-badge");
    const zone = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
    tzEl.textContent = zone;
    const jan = new Date(d.getFullYear(), 0, 15).getTimezoneOffset();
    const jul = new Date(d.getFullYear(), 6, 15).getTimezoneOffset();
    const isDst = jan !== jul && d.getTimezoneOffset() === Math.min(jan, jul);
    dstEl.hidden = !isDst;
  }

  function render() {
    const now = currentNow();
    lastMinuteKey = `${now.hour}:${now.minute}:${now.date.toDateString()}`;
    const blockNum = now.hour * 60 + now.minute + 1;
    const pctExact = ((blockNum - 1) / 1440) * 100;
    const pct = Math.floor(pctExact);

    liveTimeEl.textContent = formatTime(now.hour, now.minute);
    liveDateEl.textContent = new Intl.DateTimeFormat(undefined, {
      weekday: "long",
      day: "numeric",
      month: "long",
    }).format(now.date);

    statBlockEl.textContent = `${blockNum} / 1440`;
    statHmEl.textContent = `${pad2(now.hour)} · ${pad2(now.minute)}`;
    statPctEl.textContent = `${pct}%`;
    progressFillEl.style.width = `${pctExact.toFixed(2)}%`;
    progressTrackEl.setAttribute("aria-valuenow", String(pct));

    for (const item of cells) {
      const state = stateAt(now, item.cell);
      item.btn.dataset.state = state;
      const isSelected = selected && selected.hour === item.cell.hour && selected.minute === item.cell.minute;
      item.btn.classList.toggle("selected", Boolean(isSelected));
    }

    if (selected) {
      const selState = stateAt(now, selected);
      selEmptyEl.hidden = true;
      selDetailEl.hidden = false;
      clearBtn.hidden = false;
      selTimeEl.textContent = formatTime(selected.hour, selected.minute);
      selStateEl.textContent = stateLabel(selState);
      selBlockEl.textContent = `Block ${selected.hour * 60 + selected.minute + 1} / 1440`;
      if (selOffsetEl) {
        selOffsetEl.textContent = relativeOffset(now, selected);
      }
    } else {
      selEmptyEl.hidden = false;
      selDetailEl.hidden = true;
      clearBtn.hidden = true;
    }

    renderTimezone(now.date);
  }

  function moveSelection(key) {
    const now = currentNow();
    const base = selected || { hour: now.hour, minute: now.minute };
    const idx = base.hour * 60 + base.minute;
    let nextIdx = idx;

    if (key === "ArrowLeft") nextIdx = Math.max(0, idx - 1);
    else if (key === "ArrowRight") nextIdx = Math.min(1439, idx + 1);
    else if (key === "ArrowUp") nextIdx = Math.max(0, base.hour - 1) * 60 + base.minute;
    else if (key === "ArrowDown") nextIdx = Math.min(23, base.hour + 1) * 60 + base.minute;
    else if (key === "Home") nextIdx = base.hour * 60;
    else if (key === "End") nextIdx = base.hour * 60 + 59;

    selected = { hour: Math.floor(nextIdx / 60), minute: nextIdx % 60 };
    render();
    const targetBtn = cells[nextIdx] && cells[nextIdx].btn;
    if (targetBtn && document.activeElement && gridEl.contains(document.activeElement)) {
      targetBtn.focus();
    }
  }

  document.addEventListener("keydown", (event) => {
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    const tag = event.target && event.target.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

    if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      moveSelection(event.key);
    } else if (event.key === "j" || event.key === "J") {
      const now = currentNow();
      selected = { hour: now.hour, minute: now.minute };
      render();
    } else if (event.key === "Escape" && selected) {
      selected = null;
      render();
    }
  });

  clearBtn.addEventListener("click", () => {
    selected = null;
    render();
  });

  jumpBtn.addEventListener("click", () => {
    const now = currentNow();
    selected = { hour: now.hour, minute: now.minute };
    render();
  });

  formatBtn.addEventListener("click", () => {
    hour12 = !hour12;
    formatBtn.textContent = hour12 ? "12h" : "24h";
    render();
  });

  function tick() {
    const now = currentNow();
    const minuteKey = `${now.hour}:${now.minute}:${now.date.toDateString()}`;
    if (minuteKey !== lastMinuteKey) {
      render();
    }
  }

  detectPrimaryDownload();
  buildGrid();
  render();
  setInterval(tick, 1000);
})();
