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
    public volatile boolean knockbackTrainer;
    public volatile boolean bedwarsBridgeTrainer;
    public volatile Models.KbProfile kbProfile = new Models.KbProfile();

    public final Queue<Models.BotConfig> spawnQueue = new ConcurrentLinkedQueue<>();
    public final AtomicBoolean clearBots = new AtomicBoolean(false);
    public final ConcurrentHashMap<Integer, Models.BotCommand> botCommands = new ConcurrentHashMap<>();

    public boolean isFrost(String uuidNoDashes) {
        Models.CosmeticEntry e = cosmetics.get(uuidNoDashes);
        return e != null && e.frost;
    }
}
