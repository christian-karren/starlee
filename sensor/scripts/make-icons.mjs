import { access } from "node:fs/promises";

const requiredIcons = [
  "extension/assets/icon-16.png",
  "extension/assets/icon-16@2x.png",
  "extension/assets/icon-32.png",
  "extension/assets/icon-32@2x.png",
  "extension/assets/icon-48.png",
  "extension/assets/icon-48@2x.png",
  "extension/assets/icon-128.png",
  "extension/assets/icon-128@2x.png"
];

await Promise.all(requiredIcons.map((path) => access(path)));
