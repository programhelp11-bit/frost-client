package dev.frost.mc;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import dev.frost.core.Dispatcher;
import dev.frost.core.FrostLink;
import dev.frost.core.FrostState;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.Minecraft;

/** Shared bootstrap: starts the launcher link and drives all per-tick features. */
public final class FrostMc {
    private static FrostLink link;
    private static int ticks;
    private static String mcVersion = "?";

    private static boolean lastMenuKey;
    private static boolean lastSkinsKey;
    private static boolean lastCapesKey;

    /** Bridges the shared menu to the launcher link. */
    public static final dev.frost.core.Menu.Host HOST = new dev.frost.core.Menu.Host() {
        public void settingsChanged() {
            if (link == null) return;
            JsonObject m = new JsonObject();
            m.addProperty("type", "ui_settings");
            m.add("qol", Dispatcher.gson().toJsonTree(FrostState.INSTANCE.qol));
            m.add("hud", Dispatcher.gson().toJsonTree(FrostState.INSTANCE.hud));
            link.send(m);
        }

        public void requestBot(String style, float difficulty, String arena, boolean kit) {
            if (link == null) return;
            JsonObject m = new JsonObject();
            m.addProperty("type", "request_bot");
            m.addProperty("style", style);
            m.addProperty("difficulty", difficulty);
            m.addProperty("legacy", FrostState.INSTANCE.kbProfile.legacy);
            m.addProperty("arena", arena);
            m.addProperty("kit", kit);
            link.send(m);
        }

        public void clearBots() {
            FrostState.INSTANCE.clearBots.set(true);
            if (link == null) return;
            JsonObject m = new JsonObject();
            m.addProperty("type", "request_clear_bots");
            link.send(m);
        }

        public boolean launcherConnected() {
            return link != null && link.connected();
        }
    };

    private FrostMc() {}

    public static FrostLink link() {
        return link;
    }

    public static void init(String version) {
        mcVersion = version;
        int port = Integer.getInteger("frost.ipc.port", 25585);
        link = new FrostLink("127.0.0.1", port, FrostState.INSTANCE, FrostMc::hello);
        link.start();
        ClientTickEvents.END_CLIENT_TICK.register(FrostMc::tick);
    }

    private static JsonObject hello() {
        JsonObject o = new JsonObject();
        o.addProperty("type", "hello");
        Minecraft mc = Minecraft.getInstance();
        o.addProperty("uuid", System.getProperty("frost.uuid", ""));
        o.addProperty("name", mc.getUser().getName());
        o.addProperty("version", mcVersion);
        return o;
    }

    private static void tick(Minecraft mc) {
        ticks++;
        openMenuKey(mc);
        StatsCollector.tick(mc);
        QolApplier.tick(mc);
        TabIcons.tick(mc, ticks);
        Bots.tick(mc);

        if (ticks % 2 == 0 && link.connected() && mc.player != null) {
            JsonObject msg = new JsonObject();
            msg.addProperty("type", "stats");
            msg.add("stats", Dispatcher.gson().toJsonTree(StatsCollector.latest));
            link.send(msg);
        }
    }

    /** Right Shift = menu, R = skin wheel, X = cape wheel (polled, so no key-mapping API is needed). */
    private static void openMenuKey(Minecraft mc) {
        long window = org.lwjgl.glfw.GLFW.glfwGetCurrentContext();
        boolean menu = window != 0L
            && org.lwjgl.glfw.GLFW.glfwGetKey(window, org.lwjgl.glfw.GLFW.GLFW_KEY_RIGHT_SHIFT) == org.lwjgl.glfw.GLFW.GLFW_PRESS;
        boolean skins = window != 0L
            && org.lwjgl.glfw.GLFW.glfwGetKey(window, org.lwjgl.glfw.GLFW.GLFW_KEY_R) == org.lwjgl.glfw.GLFW.GLFW_PRESS;
        boolean capes = window != 0L
            && org.lwjgl.glfw.GLFW.glfwGetKey(window, org.lwjgl.glfw.GLFW.GLFW_KEY_X) == org.lwjgl.glfw.GLFW.GLFW_PRESS;
        if (mc.mouseHandler.isMouseGrabbed() && mc.player != null) {
            if (menu && !lastMenuKey) mc.setScreenAndShow(new FrostScreen());
            else if (skins && !lastSkinsKey) mc.setScreenAndShow(new FrostScreen("skins"));
            else if (capes && !lastCapesKey) mc.setScreenAndShow(new FrostScreen("capes"));
        }
        lastMenuKey = menu;
        lastSkinsKey = skins;
        lastCapesKey = capes;
    }

    /** Radial menu pick: apply a saved skin (launcher uploads it to the account). */
    static void requestSkin(String id) {
        if (link == null || !link.connected()) {
            FrostState.INSTANCE.notify("Frost: start the game from the Frost launcher");
            return;
        }
        JsonObject m = new JsonObject();
        m.addProperty("type", "request_skin");
        m.addProperty("id", id);
        link.send(m);
        FrostState.INSTANCE.notify("Frost: applying skin...");
    }

    /** Radial menu pick: wear a cape (frost, none or a community id). */
    static void requestCape(String id) {
        if (link == null || !link.connected()) {
            FrostState.INSTANCE.notify("Frost: start the game from the Frost launcher");
            return;
        }
        JsonObject m = new JsonObject();
        m.addProperty("type", "request_cape");
        m.addProperty("id", id);
        link.send(m);
        FrostState.INSTANCE.library.active_cape = id;
        FrostState.INSTANCE.notify("Frost: changing cape...");
    }

    static void sendTabPlayers(java.util.Collection<String> uuids) {
        JsonObject msg = new JsonObject();
        msg.addProperty("type", "tab_players");
        JsonArray arr = new JsonArray();
        uuids.forEach(arr::add);
        msg.add("uuids", arr);
        link.send(msg);
    }

    static void sendKnockbackEvent(boolean landed) {
        JsonObject msg = new JsonObject();
        msg.addProperty("type", "knockback_event");
        msg.addProperty("landed", landed);
        link.send(msg);
    }
}
