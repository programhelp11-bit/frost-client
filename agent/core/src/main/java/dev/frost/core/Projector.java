package dev.frost.core;

/** Turns world positions into screen positions (for waypoints and TNT timers) without touching the renderer. */
public final class Projector {
    private Projector() {}

    /** Result: sx, sy in GUI pixels, depth in blocks (negative = behind the camera). */
    public static final class Point {
        public double sx, sy, depth;
        public boolean onScreen;
        /** Angle in degrees to the player's right (0 = straight ahead, 90 = right, +-180 = behind). */
        public double bearing;
    }

    public static Point project(Models.View v, double wx, double wy, double wz) {
        double yaw = Math.toRadians(v.yaw), pitch = Math.toRadians(v.pitch);
        double fx = -Math.sin(yaw) * Math.cos(pitch), fy = -Math.sin(pitch), fz = Math.cos(yaw) * Math.cos(pitch);
        double rx = -Math.cos(yaw), ry = 0, rz = -Math.sin(yaw);
        // up = right x forward
        double ux = ry * fz - rz * fy, uy = rz * fx - rx * fz, uz = rx * fy - ry * fx;
        double dx = wx - v.x, dy = wy - v.y, dz = wz - v.z;
        double cz = dx * fx + dy * fy + dz * fz;
        double cx = dx * rx + dy * ry + dz * rz;
        double cy = dx * ux + dy * uy + dz * uz;
        Point p = new Point();
        p.depth = cz;
        p.bearing = Math.toDegrees(Math.atan2(cx, cz));
        double tanV = Math.tan(Math.toRadians(Math.max(10, Math.min(170, v.fov))) / 2.0);
        double scale = (v.h / 2.0) / tanV;
        if (cz > 0.05) {
            p.sx = v.w / 2.0 + cx / cz * scale;
            p.sy = v.h / 2.0 - cy / cz * scale;
            p.onScreen = p.sx >= 0 && p.sx <= v.w && p.sy >= 0 && p.sy <= v.h;
        } else {
            double len = Math.max(0.0001, Math.hypot(cx, cy));
            p.sx = v.w / 2.0 + cx / len * v.w;
            p.sy = v.h / 2.0 - cy / len * v.w;
        }
        return p;
    }

    /** Pushes a point to the border of the screen (used for arrows that point at off-screen targets). */
    public static double[] clampToEdge(Models.View v, Point p, int margin) {
        double cx = v.w / 2.0, cy = v.h / 2.0;
        double dx = p.sx - cx, dy = p.sy - cy;
        if (Math.abs(dx) < 0.001 && Math.abs(dy) < 0.001) dy = -1;
        double sx = dx == 0 ? Double.MAX_VALUE : (cx - margin) / Math.abs(dx);
        double sy = dy == 0 ? Double.MAX_VALUE : (cy - margin) / Math.abs(dy);
        double k = Math.min(sx, sy);
        return new double[] {cx + dx * k, cy + dy * k};
    }
}
