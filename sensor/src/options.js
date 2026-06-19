const form = document.querySelector("form");
const token = document.querySelector("#token");
const port = document.querySelector("#port");
const status = document.querySelector("#status");

const saved = await chrome.storage.local.get(["captureToken", "capturePort"]);
const bundled = await fetch(chrome.runtime.getURL("starlee-config.json"))
  .then((response) => response.ok ? response.json() : {})
  .catch(() => ({}));
token.value = saved.captureToken || bundled.captureToken || "";
port.value = saved.capturePort || bundled.capturePort || 47291;

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  await chrome.storage.local.set({ captureToken: token.value.trim(), capturePort: Number(port.value) });
  status.textContent = "Saved locally.";
});
