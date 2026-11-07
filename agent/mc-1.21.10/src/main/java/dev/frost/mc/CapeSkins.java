package dev.frost.mc;

import net.minecraft.client.Minecraft;
import net.minecraft.core.ClientAsset;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.entity.player.PlayerSkin;

/** Gives the player the Frost cape (bundled with the mod; called from the PlayerInfo mixin). */
public final class CapeSkins {
    private static ClientAsset.Texture frost;

    private CapeSkins() {}

    public static PlayerSkin apply(PlayerSkin base, String uuidNoDashes) {
        try {
            Minecraft mc = Minecraft.getInstance();
            if (mc.player == null || !mc.player.getUUID().toString().replace("-", "").equals(uuidNoDashes)) return base;
            if (frost == null) frost = new ClientAsset.ResourceTexture(ResourceLocation.fromNamespaceAndPath("frost_client", "cape/frost"));
            return new PlayerSkin(base.body(), frost, base.elytra(), base.model(), base.secure());
        } catch (Throwable t) {
            return base;
        }
    }
}
