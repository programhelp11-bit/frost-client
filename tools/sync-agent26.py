#!/usr/bin/env python3
"""Regenerates agent-26/mc-common from agent/mc-common (the 26.x API differs in a few names).
Run after editing agent/mc-common:  python3 tools/sync-agent26.py"""
import os, re, shutil

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
SRC = os.path.join(ROOT, 'agent', 'mc-common', 'src', 'main', 'java')
DST = os.path.join(ROOT, 'agent-26', 'mc-common', 'src', 'main', 'java')

def convert(s):
    s = s.replace('net.minecraft.client.gui.GuiGraphics;', 'net.minecraft.client.gui.GuiGraphicsExtractor;')
    s = re.sub(r'\bGuiGraphics\b', 'GuiGraphicsExtractor', s)
    s = re.sub(r'\b(g|gg)\.drawString\((.*)\);', lambda m: f'{m.group(1)}.text({m.group(2)}, false);', s)
    s = s.replace('g.renderItem(', 'g.item(')
    s = s.replace('mc.options.hideGui || ', '')
    s = s.replace('mc.screen == null', 'mc.mouseHandler.isMouseGrabbed()')
    s = s.replace('mc.screen != null', '!mc.mouseHandler.isMouseGrabbed()')
    s = s.replace('mc.setScreen(', 'mc.setScreenAndShow(')
    s = s.replace('Items.WHITE_WOOL', 'Items.COBBLESTONE')
    return s

shutil.rmtree(DST, ignore_errors=True)
for dp, _, files in os.walk(SRC):
    for f in files:
        sp = os.path.join(dp, f)
        dp2 = os.path.join(DST, os.path.relpath(dp, SRC))
        os.makedirs(dp2, exist_ok=True)
        out = convert(open(sp).read())
        if f == 'FrostScreen.java':
            # 26.x renamed Screen.render; provide both entry points (no @Override on purpose)
            out = out.replace('    public boolean isPauseScreen() {',
                '    public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {\n        render(g, mouseX, mouseY, delta);\n    }\n\n    public boolean isPauseScreen() {')
        open(os.path.join(dp2, f), 'w').write(out)
print('synced')
