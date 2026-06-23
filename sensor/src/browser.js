export function browserNameFromUserAgent(agent = "") {
  if (agent.includes("Safari/") && !agent.includes("Chrome/") && !agent.includes("Chromium/")) return "Safari";
  if (agent.includes("Edg/")) return "Edge";
  if (agent.includes("OPR/")) return "Opera";
  if (agent.includes("Brave/")) return "Brave";
  return "Chrome";
}
