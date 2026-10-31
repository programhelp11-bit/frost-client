package dev.frost.core;

public final class Util {
    private Util() {}

    private static final String[] COMPASS = {
        "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW", "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE"
    };

    /** 16-point compass label for a Minecraft yaw (0 = south, 90 = west). */
    public static String compass(float yaw) {
        float norm = ((yaw % 360f) + 360f) % 360f;
        int idx = (int) ((norm + 11.25f) / 22.5f) % 16;
        return COMPASS[idx];
    }

    /** Wrap into (-180, 180]. */
    public static float wrapDegrees(float a) {
        a %= 360f;
        if (a > 180f) a -= 360f;
        if (a <= -180f) a += 360f;
        return a;
    }
}
