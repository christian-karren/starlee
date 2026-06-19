const form = document.querySelector("form");
const token = document.querySelector("#token");
const port = document.querySelector("#port");
const status = document.querySelector("#status");

const saved = await chrome.storage.local.get(["captureToken", "capturePort"]);
token.value = saved.captureToken || "";
port.value = saved.capturePort || 47291;

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  await chrome.storage.local.set({ captureToken: token.value.trim(), capturePort: Number(port.value) });
  status.textContent = "Saved locally.";
});

