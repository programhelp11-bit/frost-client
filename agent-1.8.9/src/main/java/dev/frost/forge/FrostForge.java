package dev.frost.forge;

import dev.frost.core.Config;
import dev.frost.core.FrostState;
import net.minecraft.client.Minecraft;
import net.minecraftforge.client.event.FOVUpdateEvent;
import net.minecraftforge.client.event.RenderGameOverlayEvent;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.common.event.FMLInitializationEvent;
import net.minecraftforge.fml.common.eventhandler.SubscribeEvent;
import net.minecraftforge.fml.common.gameevent.TickEvent;

import java.io.File;

/** Frost Client for Minecraft 1.8.9 (Forge): loads the saved settings and drives all features. */
@Mod(modid = "frostclient", name = "Frost Client", version = "1.8.9", clientSideOnly = true,
    acceptedMinecraftVersions = "[1.8.9]")
public class FrostForge {
    private static File configFile;
    private int ticks;
    private static boolean lastMenuKey;

    /** The menu saves the config whenever something changes. */
    public static final dev.frost.core.Menu.Host HOST = new dev.frost.core.Menu.Host() {
        public void settingsChanged() {
            Config.save(configFile, FrostState.INSTANCE);
        }
    };

    @Mod.EventHandler
    public void init(FMLInitializationEvent e) {
        configFile = new File(new File(Minecraft.getMinecraft().mcDataDir, "config"), "frost-client.json");
        Config.load(configFile, FrostState.INSTANCE);
        MinecraftForge.EVENT_BUS.register(this);
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
        if (mc.currentScreen == null && mc.thePlayer != null && key && !lastMenuKey) mc.displayGuiScreen(new FrostScreen());
        lastMenuKey = key;
        Stats.tick(mc);
        Qol.tick(mc);
        Capes.tick(mc, ticks);
    }

    @SubscribeEvent
    public void onOverlay(RenderGameOverlayEvent.Post e) {
        if (e.type == RenderGameOverlayEvent.ElementType.ALL) Hud.render(Minecraft.getMinecraft());
    }

    @SubscribeEvent
    public void onFov(FOVUpdateEvent e) {
        Qol.fov(e);
    }
}
