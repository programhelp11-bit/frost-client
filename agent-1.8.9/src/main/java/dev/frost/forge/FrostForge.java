package dev.frost.forge;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import dev.frost.core.Dispatcher;
import dev.frost.core.FrostLink;
import dev.frost.core.FrostState;
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
            m.addProperty("legacy", true);
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
        if (key && !lastMenuKey && mc.currentScreen == null && mc.thePlayer != null) {
            mc.displayGuiScreen(new FrostScreen());
        }
        lastMenuKey = key;
        Stats.tick(mc);
        Qol.tick(mc);
        TabIcons.tick(mc, ticks);
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
}
