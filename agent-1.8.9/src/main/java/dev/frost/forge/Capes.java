package dev.frost.forge;

import net.minecraft.client.Minecraft;
import net.minecraft.client.network.NetworkPlayerInfo;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.util.ResourceLocation;
import net.minecraftforge.fml.relauncher.ReflectionHelper;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.InputStream;

/** Gives the player the Frost cape that is bundled with the mod. */
public final class Capes {
    private static ResourceLocation frost;
    private static boolean failed;

    private Capes() {}

    public static void tick(Minecraft mc, int ticks) {
        if (failed || mc.getNetHandler() == null || mc.theWorld == null || mc.thePlayer == null || ticks % 10 != 0) return;
        try {
            if (frost == null) {
                InputStream in = Capes.class.getResourceAsStream("/assets/frost_client/textures/cape/frost.png");
                if (in == null) {
                    failed = true;
                    return;
                }
                BufferedImage img = ImageIO.read(in);
                in.close();
                frost = new ResourceLocation("frost_client", "cape/frost");
                mc.getTextureManager().loadTexture(frost, new DynamicTexture(img));
            }
            NetworkPlayerInfo info = mc.getNetHandler().getPlayerInfo(mc.thePlayer.getUniqueID());
            if (info != null && info.getLocationCape() != frost) {
                ReflectionHelper.setPrivateValue(NetworkPlayerInfo.class, info, frost, "locationCape", "field_178862_f");
            }
        } catch (Throwable t) {
            failed = true;
        }
    }
}
