const PET_THEME_ROLES = ["background", "text", "accent", "muted", "border", "surface", "selection", "working", "blocked", "done", "unknown"];

function themeColors(theme) {
  return Object.fromEntries(PET_THEME_ROLES.flatMap((role) => {
    const color = theme?.colors?.[role];
    return typeof color === "string" && /^#[\da-f]{6}$/i.test(color) ? [[role, color]] : [];
  }));
}

function themeContrast(color) {
  const channels = [1, 3, 5].map((offset) => parseInt(color.slice(offset, offset + 2), 16) / 255)
    .map((value) => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4);
  const luminance = channels[0] * .2126 + channels[1] * .7152 + channels[2] * .0722;
  return (luminance + .05) / .05 >= 1.05 / (luminance + .05) ? "#000000" : "#ffffff";
}

function themeVariables(theme) {
  const colors = themeColors(theme);
  const variables = Object.fromEntries(Object.entries(colors).map(([role, color]) => [`--pet-${role}`, color]));
  if (colors.accent) variables["--pet-on-accent"] = themeContrast(colors.accent);
  if (colors.blocked) variables["--pet-on-blocked"] = themeContrast(colors.blocked);
  return variables;
}

function themedSpriteColors(base, theme) {
  const colors = themeColors(theme);
  const roles = { k: "text", w: "background", b: "accent", g: "unknown", r: "blocked", y: "working", t: "done" };
  return Object.fromEntries(Object.entries(base).map(([key, value]) => [key, colors[roles[key]] || value]));
}

if (typeof module !== "undefined") module.exports = { themeColors, themeVariables, themedSpriteColors };
