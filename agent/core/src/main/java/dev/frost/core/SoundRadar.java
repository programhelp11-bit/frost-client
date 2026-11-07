package dev.frost.core;

import java.util.ArrayList;
import java.util.List;

/** Recent sounds around the player, shown as blips on a radar (accessibility and awareness). */
public final class SoundRadar {
    public static final class Blip {
        public final String kind;
        public final double x, y, z;
        public final long at;

        Blip(String kind, double x, double y, double z, long at) {
            this.kind = kind;
            this.x = x;
            this.y = y;
            this.z = z;
            this.at = at;
        }
    }

    private static final long LIFE_MS = 2500L;
    private final List<Blip> blips = new ArrayList<Blip>();

    public synchronized void add(String kind, double x, double y, double z) {
        long now = System.currentTimeMillis();
        // the same source making the same sound again just refreshes the blip
        for (int i = blips.size() - 1; i >= 0; i--) {
            Blip b = blips.get(i);
            if (b.kind.equals(kind) && Math.abs(b.x - x) < 1.5 && Math.abs(b.z - z) < 1.5) blips.remove(i);
        }
        blips.add(new Blip(kind, x, y, z, now));
        if (blips.size() > 40) blips.remove(0);
    }

    public synchronized List<Blip> live() {
        long now = System.currentTimeMillis();
        List<Blip> out = new ArrayList<Blip>();
        for (int i = blips.size() - 1; i >= 0; i--) {
            if (now - blips.get(i).at > LIFE_MS) blips.remove(i);
        }
        out.addAll(blips);
        return out;
    }

    public static int color(String kind) {
        switch (kind) {
            case "tnt": return 0xFFFF5555;
            case "chest": return 0xFFFFAA00;
            case "potion": return 0xFFFF55FF;
            case "hurt": return 0xFFFF7755;
            case "bow": return 0xFFFFFF55;
            case "block": return 0xFFAAAAAA;
            default: return 0xFF00D2FF;   // steps
        }
    }

    /** Maps a raw sound name (1.8 or modern) to a radar category, or null if it is not interesting. */
    public static String classify(String name) {
        if (name == null) return null;
        String n = name.toLowerCase();
        if (n.contains("tnt") || n.contains("explode") || n.contains("fuse")) return "tnt";
        if (n.contains("chest") || n.contains("barrel") || n.contains("ender_chest")) return "chest";
        if (n.contains("drink") || n.contains("potion") || n.contains("splash")) return "potion";
        if (n.contains("hurt") || n.contains("hit") || n.contains("damage") || n.contains("attack")) return "hurt";
        if (n.contains("bow") || n.contains("arrow")) return "bow";
        if (n.contains("step") || n.contains("walk") || n.contains("run")) return "step";
        if (n.contains("dig") || n.contains("break") || n.contains("place") || n.contains("block")) return "block";
        return null;
    }
}
