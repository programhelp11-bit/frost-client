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

        void requestBot(String style, float difficulty, String arena, boolean kit);

        void clearBots();

        boolean launcherConnected();
    }

    public enum Tab { HUD, TRAINER, GAMEPLAY, PERFORMANCE }

    // launcher palette
    static final int TEXT = 0xFFEAF2FF;
    static final int DIM = 0xFF8FA6C8;
    static final int ACCENT = 0xFF8FD3FF;
    static final int BLUE = 0xFF3D6BFF;
    static final int LINE = 0x388CBEFF;
    static final int CARD = 0x2A12285A;
    static final int HOVER = 0x2E3D6BFF;

    public static final String[] STYLES = {"sword", "crystal", "axe_shield", "mace", "bedwars_rush"};
    public static final String[] STYLE_NAMES = {"Sword", "Crystal", "Axe & Shield", "Mace", "Bedwars rush"};

    private static final int ROW_H = 30;
    private static final int ROW_GAP = 4;

    private final Host host;
    private final FrostState state;
    public Tab tab = Tab.TRAINER;
    public int styleIdx;
    public int arenaIdx;
    public boolean kit;
    public float difficulty = 0.8f;
    public String message = "";

    private boolean wasDown;
    private boolean clicked;
    private int mx;
    private int my;
    private int scroll;

    // per-frame row layout
    private int cx, cw, rowY, curY, rowBottom, rowIdx, shown, total;

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
        wasDown = down;

        backdrop(c, sw, sh);
        int w = Math.min(sw - 24, 620);
        int h = Math.min(sh - 24, 340);
        int x = (sw - w) / 2;
        int y = (sh - h) / 2;
        int sb = 132;
        card(c, x, y, w, h, 0xC0081634, 0x4A8CBEFF);
        c.fill(x + 1, y + 1, x + w - 1, y + 3, 0x14BFE6FF);
        c.fill(x + 1, y + 1, x + sb, y + h - 1, 0xDD050B1E);
        c.fill(x + sb, y + 1, x + sb + 1, y + h - 1, LINE);

        // brand
        snowflake(c, x + 14, y + 14, ACCENT);
        c.text("F R O S T", x + 28, y + 12, TEXT);

        // navigation
        String[] names = {"HUD", "Trainer", "Gameplay", "Performance"};
        Tab[] tabs = {Tab.HUD, Tab.TRAINER, Tab.GAMEPLAY, Tab.PERFORMANCE};
        for (int i = 0; i < tabs.length; i++) {
            int ny = y + 40 + i * 26;
            boolean on = tab == tabs[i];
            boolean hov = over(x + 8, ny, sb - 16, 22);
            if (on) {
                c.fill(x + 8, ny, x + sb - 8, ny + 22, 0x335B86FF);
                c.fill(x + 8, ny + 3, x + 10, ny + 19, 0xFF5B86FF);
            } else if (hov) {
                c.fill(x + 8, ny, x + sb - 8, ny + 22, 0x1A8CBEFF);
            }
            c.text(names[i], x + 20, ny + 7, on ? TEXT : DIM);
            if (hov && clicked && !on) {
                tab = tabs[i];
                scroll = 0;
            }
        }
        c.text("Esc to close", x + 14, y + h - 18, DIM);

        // content
        cx = x + sb + 16;
        cw = w - sb - 32;
        String title = tab == Tab.HUD ? "HUD" : tab == Tab.TRAINER ? "Trainer" : tab == Tab.GAMEPLAY ? "Gameplay" : "Performance";
        c.text(title, cx, y + 14, TEXT);
        c.text(title, cx + 1, y + 14, TEXT);
        String sub = tab == Tab.HUD ? "Choose what is shown on screen."
            : tab == Tab.TRAINER ? "Practice against bots. Singleplayer only."
            : tab == Tab.GAMEPLAY ? "Quality of life while you play."
            : "Squeeze out more frames.";
        c.text(sub, cx, y + 27, DIM);

        rowY = y + 46;
        rowBottom = y + h - 34;
        rowIdx = 0;
        shown = 0;
        switch (tab) {
            case HUD: hudTab(c); break;
            case TRAINER: trainerTab(c); break;
            case GAMEPLAY: gameplayTab(c); break;
            default: performanceTab(c); break;
        }
        total = rowIdx;
        int maxScroll = Math.max(0, total - shown - (scroll > 0 ? 0 : 0));
        if (scroll > 0 || shown < total) {
            int by = y + h - 26;
            c.text((scroll + 1) + "-" + (scroll + shown) + " of " + total, x + w - 150, by + 4, DIM);
            if (smallButton(c, x + w - 62, by, 22, 16, "^") && scroll > 0) scroll--;
            if (smallButton(c, x + w - 36, by, 22, 16, "v") && scroll + shown < total) scroll++;
        }
        if (!message.isEmpty()) {
            int mw = c.width(message) + 18;
            card(c, (sw - mw) / 2, y + h + 4 > sh - 24 ? sh - 24 : y + h + 4, mw, 18, 0xF0070F26, 0x4A8CBEFF);
            c.text(message, (sw - mw) / 2 + 9, (y + h + 4 > sh - 24 ? sh - 24 : y + h + 4) + 5, ACCENT);
        }
    }

    // ---------------------------------------------------------------- backdrop

    private static int argb(int a, int r, int g, int b) {
        return (a << 24) | (r << 16) | (g << 8) | b;
    }

    private static void glow(Canvas c, int cx, int cy, int rx, int ry, int rgb, int maxAlpha) {
        int rings = 9;
        int each = Math.max(2, maxAlpha / rings);
        for (int i = 0; i < rings; i++) {
            double k = 1.0 - (double) i / rings;
            int hx = (int) (rx * k), hy = (int) (ry * k);
            if (hy < 2) break;
            int col = (each << 24) | (rgb & 0xFFFFFF);
            for (int y = -hy; y < hy; y += 3) {
                double q = (double) y / hy;
                int w = (int) (hx * Math.sqrt(Math.max(0, 1 - q * q)));
                c.fill(cx - w, cy + y, cx + w, Math.min(cy + y + 3, cy + hy), col);
            }
        }
    }

    /** The launcher's night sky: deep blue gradient, soft aurora glows, mountain ridges, drifting snow. */
    public static void backdrop(Canvas c, int sw, int sh) {
        for (int y = 0; y < sh; y += 3) {
            double t = (double) y / sh;
            c.fill(0, y, sw, Math.min(sh, y + 3), argb(0xE8, (int) (11 - 6 * t), (int) (28 - 18 * t), (int) (69 - 46 * t)));
        }
        glow(c, (int) (sw * 0.78), (int) (sh * 0.05), (int) (sw * 0.42), (int) (sh * 0.5), 0x2A56FF, 0x52);
        glow(c, (int) (sw * 0.18), (int) (sh * 0.98), (int) (sw * 0.36), (int) (sh * 0.42), 0x2FB6FF, 0x3A);
        glow(c, (int) (sw * 0.55), (int) (sh * 0.92), (int) (sw * 0.4), (int) (sh * 0.3), 0x6A4DFF, 0x30);
        // mountain ridges
        for (int x = 0; x < sw; x += 6) {
            double a = Math.sin(x * 0.021) * 0.5 + Math.sin(x * 0.047 + 1.3) * 0.3 + Math.sin(x * 0.009) * 0.6;
            int h1 = (int) (sh * (0.17 + 0.07 * a));
            double b = Math.sin(x * 0.033 + 2.1) * 0.5 + Math.sin(x * 0.015 + 0.4) * 0.5;
            int h2 = (int) (sh * (0.09 + 0.05 * b));
            c.fill(x, sh - h1, x + 6, sh, 0x8C071331);
            c.fill(x, sh - h2, x + 6, sh, 0xCC050D22);
        }
        // snow
        long t = System.currentTimeMillis();
        for (int i = 0; i < 28; i++) {
            double sp = 5 + (i * 7 % 11);
            int x = (int) ((i * 97 + Math.sin(t / 1400.0 + i) * 6 + sw * 4) % sw);
            int y = (int) (((i * 53) % sh + t / 1000.0 * sp) % sh);
            if (i % 4 == 0) snowflake(c, x, y, 0xA0BFE6FF);
            else c.fill(x, y, x + 1, y + 1, 0x90DCEBFF);
        }
    }

    // ---------------------------------------------------------------- pieces

    private boolean over(int x, int y, int w, int h) {
        return mx >= x && mx < x + w && my >= y && my < y + h;
    }

    private void card(Canvas c, int x, int y, int w, int h, int body, int border) {
        c.fill(x, y, x + w, y + h, body);
        c.fill(x, y, x + w, y + 1, border);
        c.fill(x, y + h - 1, x + w, y + h, border);
        c.fill(x, y + 1, x + 1, y + h - 1, border);
        c.fill(x + w - 1, y + 1, x + w, y + h - 1, border);
        if (h > 12) c.fill(x + 1, y + 1, x + w - 1, y + 2, 0x16BFE6FF);       // glass sheen
    }

    static void snowflake(Canvas c, int x, int y, int col) {
        int[][] px = {{2,0},{2,1},{2,2},{2,3},{2,4},{0,2},{1,2},{3,2},{4,2},{0,0},{1,1},{3,3},{4,4},{4,0},{3,1},{1,3},{0,4}};
        for (int[] p : px) c.fill(x + p[0], y + p[1], x + p[0] + 1, y + p[1] + 1, col);
    }

    private boolean smallButton(Canvas c, int x, int y, int w, int h, String label) {
        boolean hov = over(x, y, w, h);
        card(c, x, y, w, h, hov ? 0x553D6BFF : 0x3012285A, LINE);
        c.text(label, x + (w - c.width(label)) / 2, y + (h - c.lineHeight()) / 2 + 1, TEXT);
        return hov && clicked;
    }

    private String fit(Canvas c, String s, int max) {
        if (c.width(s) <= max) return s;
        while (s.length() > 1 && c.width(s + "..") > max) s = s.substring(0, s.length() - 1);
        return s + "..";
    }

    /** Starts a settings row (launcher style card). Returns false when it does not fit / is scrolled away. */
    private boolean row(Canvas c, String title, String desc, int controlW) {
        int idx = rowIdx++;
        if (idx < scroll) return false;
        if (rowY + ROW_H > rowBottom) return false;
        curY = rowY;
        rowY += ROW_H + ROW_GAP;
        shown++;
        boolean hov = over(cx, curY, cw, ROW_H);
        card(c, cx, curY, cw, ROW_H, hov ? HOVER : CARD, 0x24 << 24 | 0x8CBEFF);
        int textMax = cw - controlW - 28;
        String tt = fit(c, title, textMax);
        c.text(tt, cx + 10, curY + (desc.isEmpty() ? 11 : 5), TEXT);
        c.text(tt, cx + 11, curY + (desc.isEmpty() ? 11 : 5), TEXT);        // semi-bold like the launcher
        if (!desc.isEmpty()) c.text(fit(c, desc, textMax), cx + 10, curY + 17, DIM);
        return true;
    }

    private boolean rowToggle(Canvas c, String title, String desc, boolean on) {
        if (!row(c, title, desc, 40)) return false;
        int px = cx + cw - 10 - 28;
        int py = curY + 9;
        c.fill(px, py, px + 28, py + 12, on ? BLUE : 0x553A4A6E);
        c.fill(px, py, px + 28, py + 1, on ? 0xFF6F94FF : 0x30FFFFFF);
        int kx = on ? px + 17 : px + 2;
        c.fill(kx, py + 2, kx + 9, py + 10, 0xFFFFFFFF);
        return over(cx, curY, cw, ROW_H) && clicked;
    }

    /** -1 / 0 / +1 */
    private int rowStepper(Canvas c, String title, String desc, String value) {
        if (!row(c, title, desc, 100)) return 0;
        int right = cx + cw - 10;
        int by = curY + 8;
        int res = 0;
        if (smallButton(c, right - 16, by, 16, 14, "+")) res = 1;
        int vw = 52;
        c.text(value, right - 16 - 4 - vw / 2 - c.width(value) / 2, by + 3, TEXT);
        if (smallButton(c, right - 16 - 4 - vw - 4 - 16, by, 16, 14, "-")) res = -1;
        return res;
    }

    /** Index of the option clicked, or -1. */
    private int rowSegment(Canvas c, String title, String desc, String[] opts, int sel) {
        int totalW = 0;
        for (String o : opts) totalW += c.width(o) + 14 + 3;
        if (!row(c, title, desc, totalW)) return -1;
        int x = cx + cw - 10 - totalW + 3;
        int by = curY + 7;
        int res = -1;
        for (int i = 0; i < opts.length; i++) {
            int bw = c.width(opts[i]) + 14;
            boolean hov = over(x, by, bw, 16);
            c.fill(x, by, x + bw, by + 16, i == sel ? BLUE : (hov ? 0x553D6BFF : 0x2A8CBEFF));
            c.text(opts[i], x + 7, by + 5, TEXT);
            if (hov && clicked) res = i;
            x += bw + 3;
        }
        return res;
    }

    /** Index of the button clicked, or -1. */
    private int rowButtons(Canvas c, String title, String desc, String[] labels) {
        int totalW = 0;
        for (String o : labels) totalW += c.width(o) + 20 + 4;
        if (!row(c, title, desc, totalW)) return -1;
        int x = cx + cw - 10 - totalW + 4;
        int by = curY + 6;
        int res = -1;
        for (int i = 0; i < labels.length; i++) {
            int bw = c.width(labels[i]) + 20;
            boolean hov = over(x, by, bw, 18);
            c.fill(x, by, x + bw, by + 18, i == 0 ? (hov ? 0xFF5B86FF : BLUE) : (hov ? 0x553D6BFF : 0x2A8CBEFF));
            c.text(labels[i], x + 10, by + 6, TEXT);
            if (hov && clicked) res = i;
            x += bw + 4;
        }
        return res;
    }

    private void changed() {
        host.settingsChanged();
    }

    // ---------------------------------------------------------------- HUD

    private void hudTab(Canvas c) {
        java.util.List<Models.Widget> ws = state.hud.widgets;
        float scale = ws.isEmpty() ? 1f : ws.get(0).scale;
        int opacity = ws.isEmpty() ? 120 : ws.get(0).opacity;
        int d = rowStepper(c, "Size", "Scale of every HUD element", String.format(Locale.ROOT, "%.1fx", scale));
        if (d != 0) {
            for (Models.Widget w : ws) w.scale = Math.max(0.7f, Math.min(1.6f, Math.round((w.scale + 0.1f * d) * 10f) / 10f));
            changed();
        }
        d = rowStepper(c, "Opacity", "How see-through the panels are", Math.round(opacity / 2.55f) + "%");
        if (d != 0) {
            for (Models.Widget w : ws) w.opacity = Math.max(20, Math.min(255, w.opacity + 20 * d));
            changed();
        }
        for (Models.Widget w : ws) {
            if (rowToggle(c, label(w.kind), "", w.enabled)) {
                w.enabled = !w.enabled;
                changed();
            }
        }
    }

    // ---------------------------------------------------------------- Trainer

    private void trainerTab(Canvas c) {
        int s = rowSegment(c, "Fighting style", "Weapon and tactics of the bot", STYLE_NAMES, styleIdx);
        if (s >= 0) styleIdx = s;
        int d = rowStepper(c, "Difficulty", "Higher = faster aim, tighter spacing, better combos", Math.round(difficulty * 100) + "%");
        if (d != 0) difficulty = Math.max(0.1f, Math.min(1f, Math.round((difficulty + 0.05f * d) * 20f) / 20f));
        int a = rowSegment(c, "Arena", "Built around you, then you are teleported in", ArenaPlan.NAMES, arenaIdx);
        if (a >= 0) arenaIdx = a;
        if (rowToggle(c, "Give me a kit", "Armor, weapon, blocks and food", kit)) kit = !kit;
        int b = rowButtons(c, "Bots", "The bot learns what works against you", new String[] {"Spawn bot", "Remove all"});
        if (b == 0) {
            host.requestBot(STYLES[styleIdx], difficulty, ArenaPlan.KINDS[arenaIdx], kit);
            message = STYLE_NAMES[styleIdx] + " bot spawned";
        } else if (b == 1) {
            host.clearBots();
            message = "Bots removed";
        }
        if (rowToggle(c, "Knockback trainer", "Practice taking and dealing knockback", state.knockbackTrainer)) {
            state.knockbackTrainer = !state.knockbackTrainer;
            message = "Knockback trainer " + (state.knockbackTrainer ? "on" : "off");
        }
        if (rowToggle(c, "Bedwars bridge trainer", "Shows your bridging speed", state.bedwarsBridgeTrainer)) {
            state.bedwarsBridgeTrainer = !state.bedwarsBridgeTrainer;
            message = "Bridge trainer " + (state.bedwarsBridgeTrainer ? "on" : "off");
        }
    }

    // ---------------------------------------------------------------- Gameplay

    private void gameplayTab(Canvas c) {
        Models.Qol q = state.qol;
        if (rowToggle(c, "Fullbright", "Everything is bright, even caves", q.fullbright)) { q.fullbright = !q.fullbright; changed(); }
        if (rowToggle(c, "Minimal viewmodel", "Smaller held items", q.minimal_viewmodel)) { q.minimal_viewmodel = !q.minimal_viewmodel; changed(); }
        if (rowToggle(c, "Item physics", "Dropped items lie flat", q.item_physics)) { q.item_physics = !q.item_physics; changed(); }
        if (rowToggle(c, "Dynamic FOV", "Sprint and speed change the field of view", q.dynamic_fov)) { q.dynamic_fov = !q.dynamic_fov; changed(); }
        if (rowToggle(c, "Zoom", "Hold the zoom key", q.zoom_enabled)) { q.zoom_enabled = !q.zoom_enabled; changed(); }
        int d = rowStepper(c, "Zoom FOV", "Field of view while zooming", Math.round(q.zoom_fov) + " deg");
        if (d != 0) { q.zoom_fov = Math.max(10f, Math.min(60f, q.zoom_fov + 5f * d)); changed(); }
        if (rowToggle(c, "Hitboxes", "Show entity hitboxes", q.hitboxes)) { q.hitboxes = !q.hitboxes; changed(); }
        if (rowToggle(c, "Coordinates", "XYZ on the HUD", q.show_coordinates)) { q.show_coordinates = !q.show_coordinates; changed(); }
        if (rowToggle(c, "Biome", "Biome name on the HUD", q.show_biome)) { q.show_biome = !q.show_biome; changed(); }
    }

    // ---------------------------------------------------------------- Performance

    private void performanceTab(Canvas c) {
        Models.Qol q = state.qol;
        if (rowToggle(c, "FPS boost", "Cheaper rendering where it is not noticeable", q.fps_boost)) { q.fps_boost = !q.fps_boost; changed(); }
        if (rowToggle(c, "Fast graphics", "Flat lighting and simple leaves", q.fast_graphics)) { q.fast_graphics = !q.fast_graphics; changed(); }
        if (rowToggle(c, "Chunk optimizer", "Limits how far chunks are rebuilt", q.chunk_optimizer)) { q.chunk_optimizer = !q.chunk_optimizer; changed(); }
        int d = rowStepper(c, "Render distance cap", "Chunks, applied by the optimizer", String.valueOf(q.chunk_render_cap));
        if (d != 0) { q.chunk_render_cap = Math.max(4, Math.min(32, q.chunk_render_cap + d)); changed(); }
        if (rowToggle(c, "No clouds", "Skip cloud rendering", q.no_clouds)) { q.no_clouds = !q.no_clouds; changed(); }
        if (rowToggle(c, "No entity shadows", "Remove the shadow blob under mobs", q.no_entity_shadows)) { q.no_entity_shadows = !q.no_entity_shadows; changed(); }
        if (rowToggle(c, "Low biome blend", "Cheaper chunk meshing", q.low_biome_blend)) { q.low_biome_blend = !q.low_biome_blend; changed(); }
        if (rowToggle(c, "Cap simulation distance", "Fewer ticking chunks around you", q.cap_sim_distance)) { q.cap_sim_distance = !q.cap_sim_distance; changed(); }
        d = rowStepper(c, "Simulation distance", "Used when the cap is on", String.valueOf(q.sim_distance_cap));
        if (d != 0) { q.sim_distance_cap = Math.max(4, Math.min(16, q.sim_distance_cap + d)); changed(); }
        if (rowToggle(c, "No menu blur", "Remove the blurred menu background", q.no_menu_blur)) { q.no_menu_blur = !q.no_menu_blur; changed(); }
    }
}
