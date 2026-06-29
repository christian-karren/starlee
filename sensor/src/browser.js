export function createExtensionApi(api = globalThis.browser || globalThis.chrome) {
  if (!api) throw new Error("No WebExtension API is available.");
  const promiseApi = isPromiseStyleApi(api);
  const action = api.action || api.browserAction;
  return {
    runtime: {
      onMessage: api.runtime.onMessage,
      onStartup: api.runtime.onStartup,
      onInstalled: api.runtime.onInstalled,
      onConnect: api.runtime.onConnect,
      getURL: api.runtime.getURL.bind(api.runtime),
      getManifest: api.runtime.getManifest.bind(api.runtime),
      connect: api.runtime.connect?.bind(api.runtime),
      sendMessage: asyncMethod(api.runtime, "sendMessage", promiseApi)
    },
    storage: {
      local: {
        get: asyncMethod(api.storage.local, "get", promiseApi),
        set: asyncMethod(api.storage.local, "set", promiseApi)
      }
    },
    tabs: {
      query: asyncMethod(api.tabs, "query", promiseApi),
      sendMessage: asyncMethod(api.tabs, "sendMessage", promiseApi)
    },
    alarms: api.alarms ? {
      create: asyncMethod(api.alarms, "create", promiseApi),
      onAlarm: api.alarms.onAlarm
    } : undefined,
    action: action ? {
      onClicked: action.onClicked,
      setBadgeText: asyncMethod(action, "setBadgeText", promiseApi),
      setBadgeBackgroundColor: asyncMethod(action, "setBadgeBackgroundColor", promiseApi)
    } : undefined
  };
}

export function browserNameFromUserAgent(agent = "") {
  if (agent.includes("Firefox/")) return "Firefox";
  if (agent.includes("Safari/") && !agent.includes("Chrome/") && !agent.includes("Chromium/")) return "Safari";
  if (agent.includes("Edg/")) return "Edge";
  if (agent.includes("OPR/")) return "Opera";
  if (agent.includes("Brave/")) return "Brave";
  return "Chrome";
}

function isPromiseStyleApi(api) {
  return Boolean(api?.runtime?.getBrowserInfo) || api === globalThis.browser && api !== globalThis.chrome;
}

function asyncMethod(parent, name, promiseApi) {
  const method = parent?.[name];
  if (!method) return undefined;
  return (...args) => {
    if (promiseApi) return method.apply(parent, args);
    return new Promise((resolve, reject) => {
      try {
        const returned = method.apply(parent, [...args, (result) => {
          const error = globalThis.chrome?.runtime?.lastError;
          if (error) reject(new Error(error.message || String(error)));
          else resolve(result);
        }]);
        if (returned?.then) returned.then(resolve, reject);
      } catch (error) {
        reject(error);
      }
    });
  };
}
