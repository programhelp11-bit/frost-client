package dev.frost.core;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/** Settings and live HUD data shared by the menu, the HUD and the game hooks. */
public final class FrostState {
    public static final FrostState INSTANCE = new FrostState();

    public volatile Models.Qol qol = new Models.Qol();
    public volatile Models.HudLayout hud = Models.HudLayout.defaults();
    public volatile boolean bedwarsBridgeTrainer;
    /** True while the HUD editor is open: every enabled widget is drawn so it can be placed. */
    public volatile boolean editorOpen;
    public volatile Models.Features fx = new Models.Features();
    public final java.util.List<Models.Waypoint> waypoints = new java.util.concurrent.CopyOnWriteArrayList<Models.Waypoint>();

    /** Camera of the local player, refreshed by the game hooks every frame/tick. */
    public final Models.View view = new Models.View();
    /** Parsed BedWars scoreboard (null when not in a BedWars game). */
    public volatile BwScore bw;
    /** Primed TNT: {x, y, z, fuse ticks}. */
    public volatile java.util.List<double[]> tnts = java.util.Collections.emptyList();
    public final SoundRadar radar = new SoundRadar();
    public final ClipRecorder clip = new ClipRecorder();

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
