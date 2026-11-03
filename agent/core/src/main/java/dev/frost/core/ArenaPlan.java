package dev.frost.core;

import java.util.ArrayList;
import java.util.List;

/**
 * Block layout of the practice arenas, independent of the Minecraft version. Coordinates are
 * relative to the arena centre; dy = 0 is the floor layer, the player stands on dy = 1.
 * The version specific appliers translate {@link Mat} to real blocks.
 */
public final class ArenaPlan {
    public enum Mat { AIR, FLOOR_A, FLOOR_B, WALL, PILLAR, OBSIDIAN, END_STONE }

    public static final class Cell {
        public final int dx, dy, dz;
        public final Mat mat;

        Cell(int dx, int dy, int dz, Mat mat) {
            this.dx = dx;
            this.dy = dy;
            this.dz = dz;
            this.mat = mat;
        }
    }

    public static final String[] KINDS = {"none", "flat", "crystal", "island"};
    public static final String[] NAMES = {"No arena", "Flat", "Crystal", "Island"};

    private static final int HEIGHT = 7; // air cleared above the floor

    private ArenaPlan() {}

    public static boolean isArena(String kind) {
        return "flat".equals(kind) || "crystal".equals(kind) || "island".equals(kind);
    }

    /** Where the player is put, relative to the centre (x, y above floor, z). */
    public static int[] playerSpot(String kind) {
        return new int[] {0, 1, "island".equals(kind) ? -9 : -7};
    }

    /** Where the bot spawns, relative to the centre. */
    public static int[] botSpot(String kind) {
        return new int[] {0, 1, "island".equals(kind) ? 9 : 7};
    }

    public static List<Cell> build(String kind) {
        List<Cell> out = new ArrayList<Cell>();
        if (!isArena(kind)) return out;
        boolean island = kind.equals("island");
        int rx = island ? 5 : 12;
        int rz = island ? 14 : 12;
        for (int x = -rx; x <= rx; x++) {
            for (int z = -rz; z <= rz; z++) {
                boolean edge = Math.abs(x) == rx || Math.abs(z) == rz;
                out.add(new Cell(x, 0, z, floor(kind, x, z, rz)));
                int wallTop = kind.equals("crystal") ? 3 : 2;
                for (int y = 1; y <= HEIGHT; y++) {
                    Mat m = Mat.AIR;
                    if (!island && edge && y <= wallTop) m = kind.equals("crystal") ? Mat.OBSIDIAN : Mat.WALL;
                    out.add(new Cell(x, y, z, m));
                }
            }
        }
        if (kind.equals("crystal")) {
            int[][] pillars = {{5, 5}, {-5, 5}, {5, -5}, {-5, -5}, {0, 8}, {0, -8}, {8, 0}, {-8, 0}};
            for (int[] p : pillars) {
                for (int y = 1; y <= 4; y++) out.add(new Cell(p[0], y, p[1], Mat.PILLAR));
            }
        }
        return out;
    }

    private static Mat floor(String kind, int x, int z, int rz) {
        switch (kind) {
            case "crystal": return Mat.OBSIDIAN;
            case "island": return Math.abs(z) >= rz - 2 ? Mat.FLOOR_B : Mat.END_STONE;
            default: return ((x + z) & 1) == 0 ? Mat.FLOOR_A : Mat.FLOOR_B;
        }
    }
}
