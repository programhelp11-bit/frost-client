package dev.frost.forge;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import dev.frost.core.Dispatcher;
import dev.frost.core.FrostLink;
import dev.frost.core.FrostState;
import dev.frost.core.Models;
import net.minecraft.client.Minecraft;
import net.minecraftforge.client.event.FOVUpdateEvent;
import net.minecraftforge.client.event.RenderGameOverlayEvent;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.common.event.FMLInitializationEvent;
import net.minecraftforge.fml.common.eventhandler.SubscribeEvent;
import net.minecraftforge.fml.common.gameevent.TickEvent;

import java.util.Collection;

/** Frost Client for Minecraft 1.8.9 (Forge): bootstraps the launcher link and drives all features. */
@Mod(modid = "frostclient", name = "Frost Client", version = "1.0.0", clientSideOnly = true,
    acceptedMinecraftVersions = "[1.8.9]")
public class FrostForge {
    private static FrostLink link;
    private int ticks;

    private static boolean lastMenuKey;
    private static boolean lastSkinsKey;
    private static boolean lastCapesKey;

    /** Bridges the shared menu to the launcher link. */
    private static final java.util.concurrent.atomic.AtomicInteger BOT_IDS = new java.util.concurrent.atomic.AtomicInteger();
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
            // Bots are created right here, so they work even when the launcher link is down.
            Models.BotConfig c = new Models.BotConfig();
            c.id = BOT_IDS.incrementAndGet();
            c.name = "Frost Bot";
            c.style = style;
            c.difficulty = Math.max(0.1f, Math.min(1f, difficulty));
            c.legacy_combat = true;
            c.arena = arena == null ? "none" : arena;
            c.kit = kit;
            FrostState.INSTANCE.spawnQueue.add(c);
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

    public static FrostLink link() {
        return link;
    }

    @Mod.EventHandler
    public void init(FMLInitializationEvent e) {
        int port = Integer.getInteger("frost.ipc.port", 25585);
        link = new FrostLink("127.0.0.1", port, FrostState.INSTANCE, new java.util.function.Supplier<JsonObject>() {
            @Override
            public JsonObject get() {
                return hello();
            }
        });
        FrostState.INSTANCE.kbProfile.legacy = true;
        link.start();
        MinecraftForge.EVENT_BUS.register(this);
    }

    private static JsonObject hello() {
        JsonObject o = new JsonObject();
        o.addProperty("type", "hello");
        o.addProperty("uuid", System.getProperty("frost.uuid", ""));
        o.addProperty("name", Minecraft.getMinecraft().getSession().getUsername());
        o.addProperty("version", "1.8.9");
        return o;
    }

    @SubscribeEvent
    public void onRenderTick(TickEvent.RenderTickEvent e) {
        if (e.phase == TickEvent.Phase.START) Stats.frame(Minecraft.getMinecraft());
    }

    @SubscribeEvent
    public void onClientTick(TickEvent.ClientTickEvent e) {
        if (e.phase != TickEvent.Phase.END) return;
        Minecraft mc = Minecraft.getMinecraft();
        ticks++;
        boolean key = org.lwjgl.input.Keyboard.isKeyDown(org.lwjgl.input.Keyboard.KEY_RSHIFT);
        boolean skins = org.lwjgl.input.Keyboard.isKeyDown(org.lwjgl.input.Keyboard.KEY_R);
        boolean capes = org.lwjgl.input.Keyboard.isKeyDown(org.lwjgl.input.Keyboard.KEY_X);
        if (mc.currentScreen == null && mc.thePlayer != null) {
            if (key && !lastMenuKey) mc.displayGuiScreen(new FrostScreen());
            else if (skins && !lastSkinsKey) mc.displayGuiScreen(new FrostScreen("skins"));
            else if (capes && !lastCapesKey) mc.displayGuiScreen(new FrostScreen("capes"));
        }
        lastMenuKey = key;
        lastSkinsKey = skins;
        lastCapesKey = capes;
        Stats.tick(mc);
        Qol.tick(mc);
        TabIcons.tick(mc, ticks);
        Capes.tick(mc, ticks);
        Bots.tick(mc);
        if (ticks % 2 == 0 && link.connected() && mc.thePlayer != null) {
            JsonObject msg = new JsonObject();
            msg.addProperty("type", "stats");
            msg.add("stats", Dispatcher.gson().toJsonTree(Stats.latest));
            link.send(msg);
        }
    }

    @SubscribeEvent
    public void onOverlay(RenderGameOverlayEvent.Post e) {
        if (e.type == RenderGameOverlayEvent.ElementType.ALL) Hud.render(Minecraft.getMinecraft());
    }

    @SubscribeEvent
    public void onFov(FOVUpdateEvent e) {
        Qol.fov(e);
    }

    static void sendTabPlayers(Collection<String> uuids) {
        JsonObject msg = new JsonObject();
        msg.addProperty("type", "tab_players");
        JsonArray arr = new JsonArray();
        for (String u : uuids) arr.add(new com.google.gson.JsonPrimitive(u));
        msg.add("uuids", arr);
        link.send(msg);
    }

    static void sendKnockbackEvent(boolean landed) {
        JsonObject msg = new JsonObject();
        msg.addProperty("type", "knockback_event");
        msg.addProperty("landed", landed);
        link.send(msg);
    }

    /** Radial menu pick: apply a saved skin (the launcher uploads it to the account). */
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
}
