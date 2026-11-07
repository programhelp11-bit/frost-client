package dev.frost.core;

import java.util.List;
import java.util.Locale;

/**
 * Frost HUD elements that are drawn through the shared {@link Menu.Canvas}: the same code renders on every game
 * version. {@link #draw} paints one widget at the origin and returns its size; the game HUD handles the
 * position, scale and the editor bounds.
 */
public final class Widgets {
    static final int TEXT = 0xFFFFFFFF;
    static final int LABEL = 0xFF00D2FF;
    static final int DIM = 0xFF9FB4D6;

    private Widgets() {}

    private static final java.util.Map<String, Float> ANIM = new java.util.HashMap<String, Float>();

    private static int mix(int a, int b, float t) {
        int ar = (a >> 16) & 255, ag = (a >> 8) & 255, ab = a & 255, aa = (a >>> 24);
        int br = (b >> 16) & 255, bg = (b >> 8) & 255, bb = b & 255, ba = (b >>> 24);
        return ((int) (aa + (ba - aa) * t) << 24) | ((int) (ar + (br - ar) * t) << 16) | ((int) (ag + (bg - ag) * t) << 8) | (int) (ab + (bb - ab) * t);
    }

    /** Smoothly follows `target` (0 or 1) for the animation `id`. */
    private static float anim(String id, boolean target) {
        Float cur = ANIM.get(id);
        float v = cur == null ? (target ? 1f : 0f) : cur.floatValue();
        v += ((target ? 1f : 0f) - v) * 0.35f;
        if (Math.abs(v - (target ? 1f : 0f)) < 0.02f) v = target ? 1f : 0f;
        ANIM.put(id, Float.valueOf(v));
        return v;
    }

    /**
     * The Frost card: a frosted-glass slab. Soft ice glow, thin gradient border, a navy body that fades from
     * lighter at the top to deep at the bottom, a bright top edge, frost speckles and a tiny snow crystal in the corner.
     */
    public static void chip(Menu.Canvas c, int x, int y, int w, int h, int alpha) {
        int a = Math.max(0xC8, Math.min(0xF0, alpha));
        int r = 4;
        Menu.rfill(c, x - 1, y - 1, w + 2, h + 2, r + 1, 0x1600D2FF);                         // glow
        Menu.rfill(c, x, y, w, h, r, 0xC83FA9E6);                                              // border
        Menu.rfill(c, x + 1, y + 1, w - 2, h - 2, r - 1, (a << 24) | 0x081A3C);                // body
        int rows = Math.max(1, (h - 2) / 2);
        for (int i = 0; i < rows; i++) {                                                       // top-down sheen
            int al = 0x30 - i * 0x30 / rows;
            if (al <= 0) break;
            c.fill(x + r, y + 1 + i, x + w - r, y + 2 + i, (al << 24) | 0x5AB4FF);
        }
        c.fill(x + r, y + 1, x + w - r, y + 2, 0x70DFF4FF);                                    // bright top edge
        c.fill(x + r, y + h - 2, x + w - r, y + h - 1, 0x2800D2FF);                            // cold bottom glow
        int dots = Math.max(3, w * h / 70);
        int seed = w * 31 + h * 17;
        for (int i = 0; i < dots; i++) {                                                       // frost speckles
            seed = seed * 1103515245 + 12345;
            int px = x + 3 + ((seed >>> 8) & 0x7FFF) % Math.max(1, w - 6);
            seed = seed * 1103515245 + 12345;
            int py = y + 3 + ((seed >>> 8) & 0x7FFF) % Math.max(1, h - 6);
            c.fill(px, py, px + 1, py + 1, ((seed >>> 3) & 1) == 0 ? 0x22FFFFFF : 0x14BFEFFF);
        }
        Menu.snowflake(c, x + w - 6, y + 5, 0x66BFEFFF);
    }

    /** A glass keycap that glows ice blue while pressed. */
    private static void key(Menu.Canvas c, String id, int x, int y, int w, int h, String label, String sub, boolean down) {
        float t = anim(id, down);
        int body = mix(0xB0081A3C, 0xF000D2FF, t);
        Menu.rfill(c, x, y + 1, w, h, 3, 0x40000000);
        Menu.rfill(c, x, y, w, h, 3, mix(0xC03FA9E6, 0xFFB8F0FF, t));
        Menu.rfill(c, x + 1, y + 1, w - 2, h - 2, 2, body);
        c.fill(x + 3, y + 1, x + w - 3, y + 2, mix(0x40DFF4FF, 0x90FFFFFF, t));
        int col = mix(0xFFEAF6FF, 0xFF04142E, t);
        int ty = sub == null ? y + (h - c.lineHeight()) / 2 + 1 : y + 4;
        c.text(label, x + (w - c.width(label)) / 2, ty, col);
        if (sub != null) c.text(sub, x + (w - c.width(sub)) / 2, ty + c.lineHeight() + 1, mix(0xFF9FB4D6, 0xFF04142E, t));
    }

    private static int[] keystrokes(Menu.Canvas c, Models.Stats s) {
        int k = 18, g = 3;
        Models.Keys ks = s.keys;
        key(c, "kW", k + g, 0, k, k, "W", null, ks.w);
        key(c, "kA", 0, k + g, k, k, "A", null, ks.a);
        key(c, "kS", k + g, k + g, k, k, "S", null, ks.s);
        key(c, "kD", 2 * (k + g), k + g, k, k, "D", null, ks.d);
        int fullW = 3 * k + 2 * g;
        key(c, "kSp", 0, 2 * (k + g), fullW, 10, "", null, ks.space);
        int half = (fullW - g) / 2;
        Menu.rfill(c, fullW / 2 - 9, 2 * (k + g) + 4, 18, 2, 1, ks.space ? 0xFF04142E : 0xFF9FB4D6);   // space bar mark
        key(c, "kL", 0, 2 * (k + g) + 13, half, 25, "LMB", String.valueOf(s.cps_left), ks.lmb);
        key(c, "kR", half + g, 2 * (k + g) + 13, half, 25, "RMB", String.valueOf(s.cps_right), ks.rmb);
        return new int[] {fullW, 2 * (k + g) + 38};
    }

    private static String prettyBiome(String registered) {
        String name = registered.contains(":") ? registered.substring(registered.indexOf(':') + 1) : registered;
        StringBuilder sb = new StringBuilder();
        for (String part : name.split("_")) {
            if (part.isEmpty()) continue;
            if (sb.length() > 0) sb.append(' ');
            sb.append(Character.toUpperCase(part.charAt(0))).append(part.substring(1));
        }
        return sb.toString();
    }

    private static int[] rows(Menu.Canvas c, String[][] rows, int alpha) {
        int lw = 0, vw = 0;
        for (String[] r : rows) {
            lw = Math.max(lw, c.width(r[0]));
            vw = Math.max(vw, c.width(r[1]));
        }
        int lh = c.lineHeight() + 4;
        int width = lw + vw + 36;
        int h = rows.length * lh + 6;
        chip(c, 0, 0, width, h, alpha);
        int y = 4;
        for (int i = 0; i < rows.length; i++) {
            String[] r = rows[i];
            c.text(r[0], 9, y + 1, 0xFF8CCBF2);
            int vx = width - 15 - c.width(r[1]);
            c.text(r[1], vx, y + 1, TEXT);
            c.text(r[1], vx + 1, y + 1, TEXT);                    // bold
            if (i < rows.length - 1) c.fill(8, y + lh - 1, width - 14, y + lh, 0x1AA0DCFF);
            y += lh;
        }
        return new int[] {width, h};
    }

    /** Returns {w, h}, or null when `kind` is not one of the widgets handled here. */
    public static int[] draw(String kind, Menu.Canvas c, FrostState st, Models.Stats s, int alpha) {
        Models.Qol q = st.qol;
        switch (kind) {
            case "keystrokes": return keystrokes(c, s);
            case "cps": return rows(c, new String[][] {{"CPS", s.cps_left + " | " + s.cps_right}}, alpha);
            case "fps": return rows(c, new String[][] {{"FPS", String.valueOf(s.fps)}}, alpha);
            case "ping": return rows(c, new String[][] {{"Ping", s.ping_ms + " ms"}}, alpha);
            case "direction": return rows(c, new String[][] {{"Dir", Util.compass(s.yaw) + " " + Math.round(((s.yaw % 360f) + 360f) % 360f) + "\u00b0"}}, alpha);
            case "reach_display": return rows(c, new String[][] {{"Reach", s.last_reach == null ? "-" : String.format(Locale.ROOT, "%.2f m", s.last_reach)}}, alpha);
            case "coordinates": {
                java.util.List<String[]> r = new java.util.ArrayList<String[]>();
                if (q.show_coordinates) r.add(new String[] {"XYZ", String.format(Locale.ROOT, "%.1f %.1f %.1f", s.pos[0], s.pos[1], s.pos[2])});
                if (q.show_biome && !s.biome.isEmpty()) r.add(new String[] {"Biome", prettyBiome(s.biome)});
                return r.isEmpty() ? new int[] {0, 0} : rows(c, r.toArray(new String[0][]), alpha);
            }
            case "bw_resources": return rows(c, new String[][] {{"Iron", String.valueOf(s.bw_iron)}, {"Gold", String.valueOf(s.bw_gold)},
                {"Diamond", String.valueOf(s.bw_diamond)}, {"Emerald", String.valueOf(s.bw_emerald)}}, alpha);
            case "bw_blocks": return rows(c, new String[][] {{"Blocks", String.valueOf(s.bw_blocks)}}, alpha);
            case "bridge_speed": return rows(c, new String[][] {{"Bridge", String.format(Locale.ROOT, "%.2f b/s", s.bridge_bps)},
                {"Best", String.format(Locale.ROOT, "%.2f b/s", s.bridge_best)}}, alpha);
            case "potion_status": {
                if (s.potions.isEmpty()) return rows(c, new String[][] {{"Effects", "none"}}, alpha);
                String[][] r = new String[s.potions.size()][];
                for (int i = 0; i < r.length; i++) {
                    Models.Potion p = s.potions.get(i);
                    r[i] = new String[] {p.name, (p.remaining_secs / 60) + ":" + String.format("%02d", p.remaining_secs % 60)};
                }
                return rows(c, r, alpha);
            }
            case "bw_timers": return bwTimers(c, st, alpha);
            case "bw_events": return bwEvents(c, st, alpha);
            case "bw_beds": return bwBeds(c, st, alpha);
            case "spotify": return spotify(c, st, alpha);
            case "clip_status": return clipStatus(c, st, alpha);
            case "sound_radar": return radar(c, st, alpha);
            case "waypoints": return waypointList(c, st, alpha);
            default: return null;
        }
    }

    // ------------------------------------------------------------------ BedWars

    private static int[] bwTimers(Menu.Canvas c, FrostState st, int alpha) {
        BwScore b = st.bw;
        BwScore.Event d = b == null ? null : b.next("Diamond");
        BwScore.Event e = b == null ? null : b.next("Emerald");
        String[][] r = {
            {d == null ? "Diamond" : d.name, d == null ? "-" : BwScore.time(d.secs)},
            {e == null ? "Emerald" : e.name, e == null ? "-" : BwScore.time(e.secs)}
        };
        return rows(c, r, alpha);
    }

    private static int[] bwEvents(Menu.Canvas c, FrostState st, int alpha) {
        BwScore b = st.bw;
        if (b == null || b.events.isEmpty()) return rows(c, new String[][] {{"Next", "-"}}, alpha);
        int n = Math.min(4, b.events.size());
        String[][] r = new String[n][];
        for (int i = 0; i < n; i++) r[i] = new String[] {b.events.get(i).name, BwScore.time(b.events.get(i).secs)};
        return rows(c, r, alpha);
    }

    private static int[] bwBeds(Menu.Canvas c, FrostState st, int alpha) {
        BwScore b = st.bw;
        int lh = c.lineHeight() + 3;
        if (b == null || b.teams.isEmpty()) return rows(c, new String[][] {{"Beds", "-"}}, alpha);
        int width = 0;
        for (BwScore.Team t : b.teams) width = Math.max(width, c.width(t.name) + 14);
        int h = b.teams.size() * lh + 5;
        chip(c, 0, 0, width + 38, h + 2, alpha);
        int y = 5;
        for (BwScore.Team t : b.teams) {
            c.text(t.letter, 9, y + 1, t.color);
            c.text(t.name + (t.you ? "*" : ""), 20, y + 1, t.state == 2 ? DIM : TEXT);
            int dx = width + 24, dy = y + 2;
            if (t.state == 0) {
                Menu.rfill(c, dx, dy, 6, 6, 3, 0xFF55FF55);
            } else if (t.state == 1) {
                Menu.rfill(c, dx, dy, 6, 6, 3, 0xFFFF5555);
                if (t.players > 0) c.text(String.valueOf(t.players), dx + 9, y + 1, 0xFFFFAAAA);
            } else {
                for (int i = 0; i < 6; i++) {          // eliminated: a small cross
                    c.fill(dx + i, dy + i, dx + i + 1, dy + i + 1, 0xFFFF5555);
                    c.fill(dx + 5 - i, dy + i, dx + 6 - i, dy + i + 1, 0xFFFF5555);
                }
            }
            y += lh;
        }
        return new int[] {width + 38, h + 2};
    }

    // ------------------------------------------------------------------ Spotify

    private static String fit(Menu.Canvas c, String s, int max) {
        if (c.width(s) <= max) return s;
        while (s.length() > 1 && c.width(s + "..") > max) s = s.substring(0, s.length() - 1);
        return s + "..";
    }

    private static int[] spotify(Menu.Canvas c, FrostState st, int alpha) {
        Spotify.Track t = Spotify.track;
        int w = 124, h = 38;
        chip(c, 0, 0, w, h + 2, alpha);
        int ax = 8, ay = 5, size = 28;
        if (t != null && t.art != null) {
            int cell = size / 16 + (size % 16 > 0 ? 1 : 0);
            for (int yy = 0; yy < 16; yy++) {
                for (int xx = 0; xx < 16; xx++) {
                    int x0 = ax + xx * size / 16, y0 = ay + yy * size / 16;
                    int x1 = ax + (xx + 1) * size / 16, y1 = ay + (yy + 1) * size / 16;
                    c.fill(x0, y0, x1, y1, t.art[yy * 16 + xx]);
                }
            }
        } else {
            Menu.rfill(c, ax, ay, size, size, 3, 0xFF12285A);
            Menu.snowflake(c, ax + size / 2, ay + size / 2, LABEL);
        }
        boolean none = t == null || t.title.isEmpty();
        c.text(fit(c, none ? "Spotify" : t.title, w - 46), 42, 6, TEXT);
        c.text(fit(c, none ? "Nothing playing" : (t.artist.isEmpty() ? "" : t.artist), w - 46), 42, 6 + c.lineHeight() + 1, DIM);
        int bx = 42, by = h - 6, bw = w - 52;
        Menu.rfill(c, bx, by, bw, 3, 1, 0x553A4A6E);
        if (t != null && t.durationMs > 0) {
            int fill = (int) Math.max(0, Math.min(bw, bw * t.progressMs / t.durationMs));
            if (fill > 1) Menu.rfill(c, bx, by, fill, 3, 1, 0xFF00D2FF);
        }
        if (t != null && !t.title.isEmpty()) {
            c.text(t.playing ? "▶" : "||", w - 14, 6, t.playing ? LABEL : DIM);
        }
        return new int[] {w, h + 2};
    }

    // ------------------------------------------------------------------ Clip maker

    private static int[] clipStatus(Menu.Canvas c, FrostState st, int alpha) {
        ClipRecorder r = st.clip;
        boolean recent = System.currentTimeMillis() - r.savedAt < 4000 && !r.lastSaved.isEmpty();
        String text = recent ? "Clip saved" : (r.enabled ? "Buffer " + r.bufferedSeconds() + "s" : "Clip off");
        int w = c.width(text) + 30, h = c.lineHeight() + 9;
        chip(c, 0, 0, w, h, alpha);
        boolean blink = (System.currentTimeMillis() / 500) % 2 == 0;
        Menu.rfill(c, 9, h / 2 - 2, 5, 5, 2, recent ? 0xFF55FF55 : (r.enabled && blink ? 0xFFFF5555 : 0xFF7A3A3A));
        c.text(text, 19, 5, TEXT);
        return new int[] {w, h};
    }

    // ------------------------------------------------------------------ Sound radar

    private static int[] radar(Menu.Canvas c, FrostState st, int alpha) {
        int size = 64, r = size / 2;
        Menu.rfill(c, 0, 0, size, size, r, 0xFF2A5FA8);
        Menu.rfill(c, 1, 1, size - 2, size - 2, r - 1, 0xE0071126);
        Menu.rfill(c, r - 10, r - 10, 20, 20, 10, 0x30A0DCFF);
        c.fill(r, 4, r + 1, size - 4, 0x30FFFFFF);
        c.fill(4, r, size - 4, r + 1, 0x30FFFFFF);
        Menu.rfill(c, r - 2, r - 2, 4, 4, 2, 0xFF00D2FF);            // you
        Models.View v = st.view;
        double range = Math.max(8, st.fx.radar_range);
        for (SoundRadar.Blip b : st.radar.live()) {
            double dx = b.x - v.x, dz = b.z - v.z;
            double dist = Math.hypot(dx, dz);
            if (dist > range) continue;
            // angle relative to where the player looks; 0 = up on the radar
            double yaw = Math.toRadians(v.yaw);
            double fx = -Math.sin(yaw), fz = Math.cos(yaw);
            double rx = -Math.cos(yaw), rz = -Math.sin(yaw);
            double fwd = dx * fx + dz * fz, right = dx * rx + dz * rz;
            double k = (r - 6) * Math.min(1.0, dist / range) / Math.max(0.001, dist);
            int px = (int) Math.round(r + right * k), py = (int) Math.round(r - fwd * k);
            long age = System.currentTimeMillis() - b.at;
            int a = (int) Math.max(0x40, 0xFF - age * 0xBF / 2500L);
            int col = (a << 24) | (SoundRadar.color(b.kind) & 0xFFFFFF);
            Menu.rfill(c, px - 2, py - 2, 5, 5, 2, col);
        }
        return new int[] {size, size};
    }

    // ------------------------------------------------------------------ Waypoints

    private static int[] waypointList(Menu.Canvas c, FrostState st, int alpha) {
        List<Models.Waypoint> wp = st.waypoints;
        if (wp.isEmpty()) return rows(c, new String[][] {{"Waypoints", "none"}}, alpha);
        int n = Math.min(5, wp.size());
        String[][] r = new String[n][];
        for (int i = 0; i < n; i++) {
            Models.Waypoint w = wp.get(i);
            double d = Math.sqrt(sq(w.x - st.view.x) + sq(w.y - st.view.y) + sq(w.z - st.view.z));
            r[i] = new String[] {w.name, Math.round(d) + "m"};
        }
        return rows(c, r, alpha);
    }

    private static double sq(double a) {
        return a * a;
    }

    // ------------------------------------------------------------------ world overlays

    /** AR waypoints and TNT timers, drawn straight onto the screen in GUI coordinates (no scaling applied). */
    public static void overlays(Menu.Canvas c, FrostState st) {
        Models.View v = st.view;
        if (!v.valid) return;
        Models.Features f = st.fx;
        if (f.ar_waypoints) {
            for (Models.Waypoint w : st.waypoints) waypoint(c, v, w);
        }
        if (f.tnt_timer) {
            for (double[] t : st.tnts) {
                Projector.Point p = Projector.project(v, t[0], t[1] + 1.4, t[2]);
                if (p.depth <= 0.2 || !p.onScreen) continue;
                double secs = t[3] / 20.0;
                String s = String.format(Locale.ROOT, "%.1fs", secs);
                int col = secs < 1.5 ? 0xFFFF5555 : (secs < 3 ? 0xFFFFAA00 : 0xFFFFFFFF);
                int w = c.width(s) + 10, h = c.lineHeight() + 5;
                int x = (int) p.sx - w / 2, y = (int) p.sy - h;
                Menu.rfill(c, x, y, w, h, 3, 0xD0071126);
                c.fill(x + 3, y + h - 1, x + w - 3, y + h, col);
                c.text(s, x + 5, y + 3, col);
            }
        }
    }

    private static void waypoint(Menu.Canvas c, Models.View v, Models.Waypoint wp) {
        Projector.Point p = Projector.project(v, wp.x, wp.y + 0.5, wp.z);
        double dist = Math.sqrt(sq(wp.x - v.x) + sq(wp.y - v.y) + sq(wp.z - v.z));
        int rgb;
        try {
            rgb = Integer.parseInt(wp.color, 16) & 0xFFFFFF;
        } catch (NumberFormatException e) {
            rgb = 0x00D2FF;
        }
        int col = 0xFF000000 | rgb;
        String label = wp.name + " " + Math.round(dist) + "m";
        int dy = (int) Math.round(wp.y - v.y);
        String height = dy == 0 ? "" : (dy > 0 ? " ↑" + dy : " ↓" + (-dy));
        label += height;
        int w = c.width(label) + 10, h = c.lineHeight() + 5;
        if (p.depth > 0.3 && p.onScreen) {
            int x = (int) p.sx, y = (int) p.sy;
            // floating arrow pointing down at the target
            for (int i = 0; i < 6; i++) c.fill(x - 5 + i, y - 14 + i, x + 6 - i, y - 13 + i, col);
            c.fill(x - 1, y - 19, x + 2, y - 13, col);
            int lx = x - w / 2, ly = y - 19 - h;
            Menu.rfill(c, lx, ly, w, h, 3, 0xD0071126);
            c.fill(lx + 3, ly + h - 1, lx + w - 3, ly + h, col);
            c.text(label, lx + 5, ly + 3, 0xFFFFFFFF);
        } else {
            // off-screen: a marker on the border that points the way
            double[] e = Projector.clampToEdge(v, p, 16);
            int x = (int) e[0], y = (int) e[1];
            Menu.rfill(c, x - 5, y - 5, 11, 11, 5, col);
            Menu.rfill(c, x - 3, y - 3, 7, 7, 3, 0xFF071126);
            c.text(String.valueOf(Math.round(dist)), x - c.width(String.valueOf(Math.round(dist))) / 2, y + 7, 0xFFFFFFFF);
        }
    }

    // ------------------------------------------------------------------ crosshair

    /** Custom crosshair centred at (cx, cy). Returns false for "vanilla" (the game should draw its own). */
    public static boolean crosshair(Menu.Canvas c, Models.Features f, int cx, int cy) {
        if (f.crosshair == null || f.crosshair.equals("vanilla")) return false;
        int rgb;
        try {
            rgb = Integer.parseInt(f.crosshair_color, 16) & 0xFFFFFF;
        } catch (NumberFormatException e) {
            rgb = 0x00D2FF;
        }
        int col = 0xFF000000 | rgb;
        int size = Math.max(1, Math.min(14, f.crosshair_size)), gap = Math.max(0, Math.min(8, f.crosshair_gap));
        int shadow = 0x90000000;
        switch (f.crosshair) {
            case "dot":
                Menu.rfill(c, cx - 1, cy - 1, 3, 3, 1, col);
                break;
            case "circle": {
                int r = size + 1;
                for (int i = 0; i < 48; i++) {
                    double a = i * Math.PI * 2 / 48;
                    int x = cx + (int) Math.round(Math.cos(a) * r), y = cy + (int) Math.round(Math.sin(a) * r);
                    c.fill(x, y, x + 1, y + 1, col);
                }
                break;
            }
            default: {
                int len = size;
                bar(c, cx - gap - len, cy, len, 1, col, shadow);
                bar(c, cx + gap + 1, cy, len, 1, col, shadow);
                bar(c, cx, cy - gap - len, 1, len, col, shadow);
                bar(c, cx, cy + gap + 1, 1, len, col, shadow);
                if (f.crosshair.equals("plus_dot")) c.fill(cx, cy, cx + 1, cy + 1, col);
            }
        }
        return true;
    }

    private static void bar(Menu.Canvas c, int x, int y, int w, int h, int col, int shadow) {
        c.fill(x - 1, y - 1, x + w + 1, y + h + 1, shadow);
        c.fill(x, y, x + w, y + h, col);
    }
}
