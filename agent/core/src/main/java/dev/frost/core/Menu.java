package dev.frost.core;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * The in-game Frost menu (HUD editor, trainer, quality-of-life), written once against a tiny
 * {@link Canvas} so the Fabric 1.21.x, Fabric 26.x and Forge 1.8.9 mods share all logic.
 * Pure Java 8. Mouse input is polled by the host screen and passed to {@link #render}.
 */
public final class Menu {
    /** Minimal drawing surface supplied by each Minecraft version. */
    public interface Canvas {
        void fill(int x1, int y1, int x2, int y2, int argb);

        void text(String s, int x, int y, int argb);

        int width(String s);

        int lineHeight();
    }

    /** Actions that need the game or the launcher. */
    public interface Host {
        void settingsChanged();

        void requestBot(String style, float difficulty);

        void clearBots();

        boolean launcherConnected();
    }

    public enum Tab { HUD, TRAINER, QUALITY }

    static final int BG = 0xCC0A1838;
    static final int PANEL = 0xD90E204E;
    static final int BORDER = 0xAA6EAAFF;
    static final int HOVER = 0x553D6BFF;
    static final int ACTIVE = 0xB43D6BFF;
    static final int TEXT = 0xFFEAF2FF;
    static final int DIM = 0xFFA0AFC8;
    static final int ACCENT = 0xFF8FD3FF;

    public static final String[] STYLES = {"sword", "crystal", "axe_shield", "mace", "bedwars_rush"};
    public static final String[] STYLE_NAMES = {"Sword", "Crystal", "Axe & Shield", "Mace", "Bedwars rush"};

    private final Host host;
    private final FrostState state;
    public Tab tab = Tab.HUD;
    public String selected = "cps";
    public int styleIdx;
    public float difficulty = 0.8f;
    public String message = "";

    private boolean wasDown;
    private boolean clicked;
    private boolean released;
    private int mx;
    private int my;
    private String dragging;
    private int dragDx;
    private int dragDy;

    public Menu(Host host, FrostState state) {
        this.host = host;
        this.state = state;
    }

    public static String label(String kind) {
        switch (kind) {
            case "cps": return "CPS";
            case "fps": return "FPS";
            case "ping": return "Ping";
            case "keystrokes": return "Keystrokes";
            case "armor_status": return "Armor";
            case "potion_status": return "Potions";
            case "direction": return "Direction";
            case "reach_display": return "Reach";
            case "coordinates": return "Coordinates";
            case "bw_resources": return "Bedwars resources";
            case "bw_blocks": return "Bedwars blocks";
            case "bridge_speed": return "Bridge speed";
            default: return kind;
        }
    }

    // ---------------------------------------------------------------- frame

    public void render(Canvas c, int sw, int sh, int mouseX, int mouseY, boolean down) {
        mx = mouseX;
        my = mouseY;
        clicked = down && !wasDown;
        released = !down && wasDown;
        wasDown = down;

        c.fill(0, 0, sw, sh, 0x59050C20);
        dock(c, sw);
        switch (tab) {
            case HUD: hudTab(c, sw, sh); break;
            case TRAINER: trainerTab(c, sw, sh); break;
            default: qualityTab(c, sw, sh); break;
        }
        if (!message.isEmpty()) {
            int w = c.width(message) + 16;
            panel(c, (sw - w) / 2, sh - 44, w, 18);
            c.text(message, (sw - w) / 2 + 8, sh - 40, ACCENT);
        }
        if (released) dragging = null;
    }

    // ---------------------------------------------------------------- pieces

    private boolean over(int x, int y, int w, int h) {
        return mx >= x && mx < x + w && my >= y && my < y + h;
    }

    private void panel(Canvas c, int x, int y, int w, int h) {
        c.fill(x, y, x + w, y + h, PANEL);
        border(c, x, y, w, h, BORDER);
    }

    private void border(Canvas c, int x, int y, int w, int h, int col) {
        c.fill(x, y, x + w, y + 1, col);
        c.fill(x, y + h - 1, x + w, y + h, col);
        c.fill(x, y + 1, x + 1, y + h - 1, col);
        c.fill(x + w - 1, y + 1, x + w, y + h - 1, col);
    }

    static void snowflake(Canvas c, int x, int y, int col) {
        int[][] px = {{2,0},{2,1},{2,2},{2,3},{2,4},{0,2},{1,2},{3,2},{4,2},{0,0},{1,1},{3,3},{4,4},{4,0},{3,1},{1,3},{0,4}};
        for (int[] p : px) c.fill(x + p[0], y + p[1], x + p[0] + 1, y + p[1] + 1, col);
    }

    /** Flat button; returns true on the frame it was clicked. */
    private boolean button(Canvas c, int x, int y, int w, int h, String label, boolean active) {
        boolean hov = over(x, y, w, h);
        c.fill(x, y, x + w, y + h, active ? ACTIVE : (hov ? HOVER : 0x660E204E));
        border(c, x, y, w, h, active ? 0xFFBFE6FF : 0x886EAAFF);
        c.text(label, x + (w - c.width(label)) / 2, y + (h - c.lineHeight()) / 2 + 1, TEXT);
        return hov && clicked;
    }

    private boolean toggle(Canvas c, int x, int y, int w, String label, boolean on) {
        int h = 14;
        boolean hov = over(x, y, w, h);
        if (hov) c.fill(x, y, x + w, y + h, HOVER);
        c.fill(x + 2, y + 2, x + 12, y + 12, 0x990E204E);
        border(c, x + 2, y + 2, 10, 10, on ? 0xFFBFE6FF : 0x886EAAFF);
        if (on) c.fill(x + 4, y + 4, x + 10, y + 10, ACCENT);
        c.text(label, x + 18, y + 3, on ? TEXT : DIM);
        return hov && clicked;
    }

    private void dock(Canvas c, int sw) {
        String[] names = {"HUD", "Trainer", "Quality"};
        Tab[] tabs = {Tab.HUD, Tab.TRAINER, Tab.QUALITY};
        int tw = 70;
        int total = 70 + tw * 3 + 8;
        int x = (sw - total) / 2;
        int y = 8;
        panel(c, x, y, total, 22);
        snowflake(c, x + 8, y + 8, ACCENT);
        c.text("FROST", x + 18, y + 7, TEXT);
        for (int i = 0; i < 3; i++) {
            int bx = x + 70 + 4 + i * tw;
            if (button(c, bx + 2, y + 3, tw - 4, 16, names[i], tab == tabs[i])) tab = tabs[i];
        }
    }

    // ---------------------------------------------------------------- HUD tab

    private Models.Widget widget(String kind) {
        for (Models.Widget w : state.hud.widgets) if (w.kind.equals(kind)) return w;
        return null;
    }

    private void hudTab(Canvas c, int sw, int sh) {
        // widget list
        int lx = 8;
        int ly = 40;
        int lw = 138;
        List<Models.Widget> ws = state.hud.widgets;
        panel(c, lx, ly, lw, 18 + ws.size() * 14 + 6);
        c.text("Widgets", lx + 8, ly + 5, DIM);
        int y = ly + 18;
        for (Models.Widget w : ws) {
            if (toggle(c, lx + 4, y, lw - 8, label(w.kind), w.enabled)) {
                w.enabled = !w.enabled;
                selected = w.kind;
                host.settingsChanged();
            } else if (over(lx + 4, y, lw - 8, 14) && clicked) {
                selected = w.kind;
            }
            if (w.kind.equals(selected)) border(c, lx + 1, y, lw - 2, 14, ACCENT);
            y += 14;
        }

        // inspector
        Models.Widget sel = widget(selected);
        int ix = sw - 158;
        int iy = 40;
        int iw = 150;
        panel(c, ix, iy, iw, 112);
        if (sel != null) {
            c.text(label(sel.kind), ix + 8, iy + 6, ACCENT);
            c.text("Scale", ix + 8, iy + 24, DIM);
            if (button(c, ix + 70, iy + 20, 16, 14, "-", false)) { sel.scale = Math.max(0.5f, round1(sel.scale - 0.1f)); host.settingsChanged(); }
            c.text(String.format(Locale.ROOT, "%.1f", sel.scale), ix + 91, iy + 24, TEXT);
            if (button(c, ix + 120, iy + 20, 16, 14, "+", false)) { sel.scale = Math.min(3f, round1(sel.scale + 0.1f)); host.settingsChanged(); }
            c.text("Opacity", ix + 8, iy + 44, DIM);
            if (button(c, ix + 70, iy + 40, 16, 14, "-", false)) { sel.opacity = Math.max(0, sel.opacity - 20); host.settingsChanged(); }
            c.text(Math.round(sel.opacity / 2.55f) + "%", ix + 91, iy + 44, TEXT);
            if (button(c, ix + 120, iy + 40, 16, 14, "+", false)) { sel.opacity = Math.min(255, sel.opacity + 20); host.settingsChanged(); }
            c.text("Grid", ix + 8, iy + 64, DIM);
            if (button(c, ix + 70, iy + 60, 16, 14, "-", false)) { state.hud.snap_grid = Math.max(1f, state.hud.snap_grid - 1f); host.settingsChanged(); }
            c.text(String.valueOf(Math.round(state.hud.snap_grid)), ix + 91, iy + 64, TEXT);
            if (button(c, ix + 120, iy + 60, 16, 14, "+", false)) { state.hud.snap_grid = Math.min(32f, state.hud.snap_grid + 1f); host.settingsChanged(); }
            if (button(c, ix + 8, iy + 84, iw - 16, 18, "Reset position", false)) {
                sel.pos = new float[] {6f, 6f};
                host.settingsChanged();
            }
        }
        c.text("Drag a widget to move it", sw / 2 - c.width("Drag a widget to move it") / 2, sh - 24, DIM);

        // dragging (skip while the pointer is on one of our panels)
        boolean onUi = over(lx, ly, lw, 18 + ws.size() * 14 + 6) || over(ix, iy, iw, 112) || over(0, 0, sw, 32);
        if (clicked && !onUi) {
            for (int i = ws.size() - 1; i >= 0; i--) {
                Models.Widget w = ws.get(i);
                int[] b = w.enabled ? state.bounds.get(w.kind) : null;
                if (b != null && over(b[0], b[1], Math.max(b[2], 8), Math.max(b[3], 8))) {
                    dragging = w.kind;
                    selected = w.kind;
                    dragDx = mx - b[0];
                    dragDy = my - b[1];
                    break;
                }
            }
        }
        if (dragging != null && wasDown) {
            Models.Widget w = widget(dragging);
            if (w != null) {
                float g = Math.max(1f, state.hud.snap_grid);
                float nx = Math.max(0, Math.min(sw - 10, mx - dragDx));
                float ny = Math.max(0, Math.min(sh - 10, my - dragDy));
                w.pos = new float[] {Math.round(nx / g) * g, Math.round(ny / g) * g};
            }
        }
        if (released && dragging != null) host.settingsChanged();
        // outline of the selected widget
        int[] sb = state.bounds.get(selected);
        if (sb != null && sel != null && sel.enabled) border(c, sb[0] - 2, sb[1] - 2, sb[2] + 4, sb[3] + 4, ACCENT);
    }

    private static float round1(float v) {
        return Math.round(v * 10f) / 10f;
    }

    // ---------------------------------------------------------------- Trainer tab

    private void trainerTab(Canvas c, int sw, int sh) {
        int w = 300;
        int h = 150;
        int x = (sw - w) / 2;
        int y = 44;
        panel(c, x, y, w, h);
        c.text("Combat bots", x + 8, y + 6, ACCENT);
        int bx = x + 8;
        for (int i = 0; i < STYLES.length; i++) {
            int bw = c.width(STYLE_NAMES[i]) + 10;
            if (bx + bw > x + w - 8) break;
            if (button(c, bx, y + 20, bw, 16, STYLE_NAMES[i], styleIdx == i)) styleIdx = i;
            bx += bw + 3;
        }
        c.text("Difficulty", x + 8, y + 46, DIM);
        if (button(c, x + 80, y + 42, 16, 14, "-", false)) difficulty = Math.max(0.1f, round05(difficulty - 0.05f));
        c.text(Math.round(difficulty * 100) + "%", x + 102, y + 46, TEXT);
        if (button(c, x + 134, y + 42, 16, 14, "+", false)) difficulty = Math.min(1f, round05(difficulty + 0.05f));
        if (button(c, x + 8, y + 64, 90, 18, "Spawn bot", false)) {
            if (host.launcherConnected()) {
                host.requestBot(STYLES[styleIdx], difficulty);
                message = STYLE_NAMES[styleIdx] + " bot requested";
            } else {
                message = "Start the game from the Frost launcher to use bots";
            }
        }
        if (button(c, x + 104, y + 64, 90, 18, "Remove bots", false)) {
            host.clearBots();
            message = "Bots removed";
        }
        c.text(host.launcherConnected() ? "Launcher connected" : "Launcher not connected", x + 8, y + 90, host.launcherConnected() ? ACCENT : 0xFFFF8A9A);

        if (toggle(c, x + 6, y + 108, w - 12, "Knockback trainer", state.knockbackTrainer)) {
            state.knockbackTrainer = !state.knockbackTrainer;
            message = "Knockback trainer " + (state.knockbackTrainer ? "on" : "off");
        }
        if (toggle(c, x + 6, y + 124, w - 12, "Bedwars bridge trainer", state.bedwarsBridgeTrainer)) {
            state.bedwarsBridgeTrainer = !state.bedwarsBridgeTrainer;
            message = "Bridge trainer " + (state.bedwarsBridgeTrainer ? "on" : "off");
        }
    }

    private static float round05(float v) {
        return Math.round(v * 20f) / 20f;
    }

    // ---------------------------------------------------------------- Quality tab

    private void qualityTab(Canvas c, int sw, int sh) {
        Models.Qol q = state.qol;
        int w = 320;
        int h = 22 + 5 * 14 + 30;
        int x = (sw - w) / 2;
        int y = 44;
        panel(c, x, y, w, h);
        c.text("Quality of life", x + 8, y + 6, ACCENT);
        int cw = (w - 12) / 2;
        int row = y + 22;
        if (toggle(c, x + 6, row, cw, "Fullbright", q.fullbright)) { q.fullbright = !q.fullbright; host.settingsChanged(); }
        if (toggle(c, x + 6 + cw, row, cw, "Minimal viewmodel", q.minimal_viewmodel)) { q.minimal_viewmodel = !q.minimal_viewmodel; host.settingsChanged(); }
        row += 14;
        if (toggle(c, x + 6, row, cw, "Item physics", q.item_physics)) { q.item_physics = !q.item_physics; host.settingsChanged(); }
        if (toggle(c, x + 6 + cw, row, cw, "Dynamic FOV", q.dynamic_fov)) { q.dynamic_fov = !q.dynamic_fov; host.settingsChanged(); }
        row += 14;
        if (toggle(c, x + 6, row, cw, "Zoom", q.zoom_enabled)) { q.zoom_enabled = !q.zoom_enabled; host.settingsChanged(); }
        if (toggle(c, x + 6 + cw, row, cw, "Hitboxes", q.hitboxes)) { q.hitboxes = !q.hitboxes; host.settingsChanged(); }
        row += 14;
        if (toggle(c, x + 6, row, cw, "Coordinates", q.show_coordinates)) { q.show_coordinates = !q.show_coordinates; host.settingsChanged(); }
        if (toggle(c, x + 6 + cw, row, cw, "Biome", q.show_biome)) { q.show_biome = !q.show_biome; host.settingsChanged(); }
        row += 14;
        if (toggle(c, x + 6, row, cw, "FPS boost", q.fps_boost)) { q.fps_boost = !q.fps_boost; host.settingsChanged(); }
        if (toggle(c, x + 6 + cw, row, cw, "Chunk optimizer", q.chunk_optimizer)) { q.chunk_optimizer = !q.chunk_optimizer; host.settingsChanged(); }
        row += 20;
        c.text("Zoom FOV", x + 8, row + 3, DIM);
        if (button(c, x + 80, row, 16, 14, "-", false)) { q.zoom_fov = Math.max(10f, q.zoom_fov - 5f); host.settingsChanged(); }
        c.text(Math.round(q.zoom_fov) + "°", x + 102, row + 3, TEXT);
        if (button(c, x + 134, row, 16, 14, "+", false)) { q.zoom_fov = Math.min(60f, q.zoom_fov + 5f); host.settingsChanged(); }
    }
}
