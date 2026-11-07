package dev.frost.core;

import java.util.Map;
import java.util.Queue;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicBoolean;

/** State pushed by the launcher; written by the IPC thread, read by the game thread. */
public final class FrostState {
    public static final FrostState INSTANCE = new FrostState();

    public volatile Models.Qol qol = new Models.Qol();
    public volatile Models.HudLayout hud = Models.HudLayout.defaults();
    public volatile Map<String, Models.CosmeticEntry> cosmetics = java.util.Collections.emptyMap();
    public volatile Models.Library library = new Models.Library();
    public volatile boolean knockbackTrainer;
    public volatile boolean bedwarsBridgeTrainer;
    public volatile Models.KbProfile kbProfile = new Models.KbProfile();

    public final Queue<Models.BotConfig> spawnQueue = new ConcurrentLinkedQueue<>();
    public final AtomicBoolean clearBots = new AtomicBoolean(false);
    public final ConcurrentHashMap<Integer, Models.BotCommand> botCommands = new ConcurrentHashMap<>();

    /** Last drawn bounds {x, y, w, h} of every HUD widget, filled by the renderer, used by the menu. */
    public final Map<String, int[]> bounds = new ConcurrentHashMap<String, int[]>();

    /** Short on-screen notice (drawn by the HUD for a few seconds). */
    /** PNG of the custom cape the player wears (null = built-in Frost cape). */
    public volatile byte[] ownCapePng;
    /** Tactics trained by the launcher (see botlab.rs); null until the launcher sends them. */
    public volatile float[] botParams;
    public volatile int botGeneration;
    public volatile String notice = "";
    public volatile long noticeUntil;

    public void notify(String text) {
        notice = text;
        noticeUntil = System.currentTimeMillis() + 3500L;
    }

    public boolean isFrost(String uuidNoDashes) {
        Models.CosmeticEntry e = cosmetics.get(uuidNoDashes);
        return e != null && e.frost;
    }
}
