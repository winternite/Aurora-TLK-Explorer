function isAuroraWindow(window) {
    const resourceClass = String(window.resourceClass).toLowerCase();
    return resourceClass.includes("auroratlkexplorer")
        || resourceClass.includes("aurora_tools")
        || String(window.caption).endsWith("Aurora TLK Explorer");
}

function placeAuroraWindow(window) {
    if (!window || !window.normalWindow || !isAuroraWindow(window)) {
        return;
    }

    const output = workspace.screenAt(workspace.cursorPos);
    if (!output) {
        return;
    }

    workspace.sendClientToScreen(window, output);
    const area = output.geometry;
    const geometry = window.frameGeometry;

    // KWin owns fullscreen/maximized geometry. Re-centering either state
    // creates a visible strip of wallpaper around the restored window.
    const fillsOutput = geometry.width >= area.width * 0.9
        && geometry.height >= area.height * 0.9;
    if (window.fullScreen || fillsOutput) {
        return;
    }

    geometry.x = area.x + Math.max(0, (area.width - geometry.width) / 2);
    geometry.y = area.y + Math.max(0, (area.height - geometry.height) / 2);
    window.frameGeometry = geometry;
}

workspace.windowAdded.connect(placeAuroraWindow);
for (const window of workspace.stackingOrder) {
    placeAuroraWindow(window);
}
