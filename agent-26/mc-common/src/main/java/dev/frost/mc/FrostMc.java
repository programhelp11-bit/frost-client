package dev.frost.mc;

import dev.frost.core.Config;
import dev.frost.core.FrostState;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;

/** Shared bootstrap: loads the saved settings and drives all per-tick features. */
public final class FrostMc {
    private static java.io.File configFile;
    private static boolean lastMenuKey;

    /** The menu saves the config whenever something changes. */
    public static final dev.frost.core.Menu.Host HOST = new dev.frost.core.Menu.Host() {
        public void settingsChanged() {
            Config.save(configFile, FrostState.INSTANCE);
        }
    };

    private FrostMc() {}

    public static void init(String version) {
        configFile = FabricLoader.getInstance().getConfigDir().resolve("frost-client.json").toFile();
        Config.load(configFile, FrostState.INSTANCE);
        ClientTickEvents.END_CLIENT_TICK.register(FrostMc::tick);
    }

    private static void tick(Minecraft mc) {
        openMenuKey(mc);
        StatsCollector.tick(mc);
        QolApplier.tick(mc);
    }

    /** Right Shift opens the Frost menu (polled, so no key-mapping API is needed). */
    private static void openMenuKey(Minecraft mc) {
        long window = org.lwjgl.glfw.GLFW.glfwGetCurrentContext();
        boolean menu = window != 0L
            && org.lwjgl.glfw.GLFW.glfwGetKey(window, org.lwjgl.glfw.GLFW.GLFW_KEY_RIGHT_SHIFT) == org.lwjgl.glfw.GLFW.GLFW_PRESS;
        if (mc.mouseHandler.isMouseGrabbed() && mc.player != null && menu && !lastMenuKey) mc.setScreenAndShow(new FrostScreen());
        lastMenuKey = menu;
    }
}
