package dev.frost.core;

import java.util.ArrayList;
import java.util.List;

/**
 * Frost radial menu (skins on R, capes on X): a ring split into equal slots, drawn with filled
 * scanline runs so it needs nothing but {@link Menu.Canvas#fill}. Pure logic, version independent.
 */
public final class Radial {
    public static final class Slot {
        public final String id;
        public final String name;
        public final int color;

        public Slot(String id, String name, int color) {
            this.id = id;
            this.name = name;
            this.color = color;
        }
    }

    private static final int TEXT = 0xFFEAF2FF;
    private static final int DIM = 0xFFA0AFC8;
    private static final int ACCENT = 0xFF8FD3FF;
    private static final int RING = 0xA00E204E;
    private static final int RING_ALT = 0xA0122A5C;
    private static final int HOVER = 0xD23D6BFF;
    private static final int ACTIVE = 0xB48FD3FF;

    private final String title;
    private final String hint;
    private final List<Slot> slots;
    private final String activeId;
    private boolean wasDown = true; // the opening click must not select anything

    public Radial(String title, String hint, List<Slot> slots, String activeId) {
        this.title = title;
        this.hint = hint;
        this.slots = new ArrayList<Slot>(slots);
        this.activeId = activeId == null ? "" : activeId;
    }

    public static int parseColor(String hex) {
        try {
            return 0xFF000000 | (Integer.parseInt(hex.trim(), 16) & 0xFFFFFF);
        } catch (RuntimeException e) {
            return 0xFF5A8CFF;
        }
    }

    /** Slot under the offset (dx, dy) from the centre, or -1 (inside the hole / outside the ring). */
    public static int indexAt(int n, double dx, double dy, double inner, double outer) {
        if (n <= 0) return -1;
        double d = Math.sqrt(dx * dx + dy * dy);
        if (d < inner || d > outer) return -1;
        double a = Math.atan2(dy, dx) + Math.PI / 2; // 0 at the top, clockwise
        if (a < 0) a += 2 * Math.PI;
        int i = (int) (a / (2 * Math.PI / n));
        return Math.min(i, n - 1);
    }

    /** Draws the ring; returns the id of the slot clicked this frame, or null. */
    public String render(Menu.Canvas c, int sw, int sh, int mouseX, int mouseY, boolean down) {
        boolean clicked = down && !wasDown;
        wasDown = down;
        int n = slots.size();
        int cx = sw / 2;
        int cy = sh / 2;
        int outer = Math.max(50, Math.min(110, Math.min(sw, sh) / 2 - 24));
        int inner = outer * 38 / 100;
        c.fill(0, 0, sw, sh, 0x66050C20);

        int hover = indexAt(n, mouseX - cx, mouseY - cy, inner, outer);
        if (n == 0) {
            drawEmpty(c, cx, cy);
            return null;
        }
        double sector = 2 * Math.PI / n;
        for (int y = -outer; y <= outer; y++) {
            double xo = Math.sqrt((double) outer * outer - (double) y * y);
            int xOuter = (int) xo;
            int xInner = Math.abs(y) < inner ? (int) Math.sqrt((double) inner * inner - (double) y * y) : -1;
            if (xInner < 0) {
                run(c, cx, cy, y, -xOuter, xOuter, n, sector, hover, outer);
            } else {
                run(c, cx, cy, y, -xOuter, -xInner - 1, n, sector, hover, outer);
                run(c, cx, cy, y, xInner + 1, xOuter, n, sector, hover, outer);
            }
        }
        // labels
        double mid = (inner + outer) / 2.0 - 4;
        for (int i = 0; i < n; i++) {
            double ang = -Math.PI / 2 + sector * (i + 0.5);
            String name = fit(slots.get(i).name, 12);
            int tx = cx + (int) Math.round(Math.cos(ang) * mid) - c.width(name) / 2;
            int ty = cy + (int) Math.round(Math.sin(ang) * mid) - c.lineHeight() / 2;
            c.text(name, tx, ty, i == hover ? 0xFFFFFFFF : TEXT);
        }
        // centre
        Menu.snowflake(c, cx - 2, cy - 18, ACCENT);
        String centre = hover >= 0 ? slots.get(hover).name : title;
        c.text(centre, cx - c.width(centre) / 2, cy - 4, hover >= 0 ? TEXT : ACCENT);
        c.text(hint, cx - c.width(hint) / 2, cy + 8, DIM);

        if (clicked && hover >= 0) return slots.get(hover).id;
        return null;
    }

    private void drawEmpty(Menu.Canvas c, int cx, int cy) {
        Menu.snowflake(c, cx - 2, cy - 20, ACCENT);
        c.text(title, cx - c.width(title) / 2, cy - 6, ACCENT);
        String[] lines = hint.split("\n");
        int y = cy + 6;
        for (String l : lines) {
            c.text(l, cx - c.width(l) / 2, y, DIM);
            y += c.lineHeight() + 2;
        }
    }

    /** One horizontal span of the ring, split into runs of equal colour. */
    private void run(Menu.Canvas c, int cx, int cy, int y, int x1, int x2, int n, double sector, int hover, int outer) {
        if (x2 < x1) return;
        int start = x1;
        int cur = 0;
        boolean have = false;
        for (int x = x1; x <= x2 + 1; x++) {
            int col = x <= x2 ? colorAt(x, y, n, sector, hover, outer) : 0;
            if (!have) {
                cur = col;
                have = true;
                start = x;
                continue;
            }
            if (col != cur) {
                if (cur != 0) c.fill(cx + start, cy + y, cx + x, cy + y + 1, cur);
                cur = col;
                start = x;
            }
        }
    }

    private int colorAt(int x, int y, int n, double sector, int hover, int outer) {
        double a = Math.atan2(y, x) + Math.PI / 2;
        if (a < 0) a += 2 * Math.PI;
        int i = Math.min((int) (a / sector), n - 1);
        double r = Math.sqrt((double) x * x + (double) y * y);
        double within = (a - i * sector) * r; // arc length from the slot's start edge
        double len = sector * r;
        if (n > 1 && (within < 1.5 || within > len - 1.5)) return 0; // gap between slots
        Slot s = slots.get(i);
        if (r > outer - 5 && r <= outer - 1) return s.color; // swatch band on the rim
        if (i == hover) return HOVER;
        if (s.id.equals(activeId)) return ACTIVE;
        return (i & 1) == 0 ? RING : RING_ALT;
    }

    private static String fit(String s, int max) {
        return s.length() <= max ? s : s.substring(0, max - 1) + ".";
    }
}
