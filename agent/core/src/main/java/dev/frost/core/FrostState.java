package dev.frost.core;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/** Settings and live HUD data shared by the menu, the HUD and the game hooks. */
public final class FrostState {
    public static final FrostState INSTANCE = new FrostState();

    public volatile Models.Qol qol = new Models.Qol();
    public volatile Models.HudLayout hud = Models.HudLayout.defaults();
    public volatile boolean bedwarsBridgeTrainer;

    /** Last drawn bounds {x, y, w, h} of every HUD widget, filled by the renderer, used by the HUD editor. */
    public final Map<String, int[]> bounds = new ConcurrentHashMap<String, int[]>();

    /** Short on-screen notice (drawn by the HUD for a few seconds). */
    public volatile String notice = "";
    public volatile long noticeUntil;

    public void notify(String text) {
        notice = text;
        noticeUntil = System.currentTimeMillis() + 3500L;
    }
}
