package dev.frost.mc;

import com.mojang.blaze3d.platform.NativeImage;
import dev.frost.core.FrostState;
import dev.frost.core.Models;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.core.ClientAsset;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.entity.player.PlayerSkin;

/** Swaps in the cape a Frost user picked (called from the PlayerInfo mixin, render thread). */
public final class CapeSkins {
    private static final Map<String, ClientAsset.Texture> TEXTURES = new ConcurrentHashMap<>();
    private static final Map<String, Boolean> BROKEN = new ConcurrentHashMap<>();

    private CapeSkins() {}

    public static PlayerSkin apply(PlayerSkin base, String uuidNoDashes) {
        try {
            Minecraft mc = Minecraft.getInstance();
            if (mc.player != null && mc.player.getUUID().toString().replace("-", "").equals(uuidNoDashes)) {
                // Own cape: bundled with the mod, so it works without any server.
                if ("none".equals(FrostState.INSTANCE.library.active_cape)) return base;
                ClientAsset.Texture own = TEXTURES.get("own");
                if (own == null) {
                    own = new ClientAsset.ResourceTexture(ResourceLocation.fromNamespaceAndPath("frost_client", "cape/frost"));
                    TEXTURES.put("own", own);
                }
                return new PlayerSkin(base.body(), own, base.elytra(), base.model(), base.secure());
            }
            Models.CosmeticEntry e = FrostState.INSTANCE.cosmetics.get(uuidNoDashes);
            if (e == null || !e.frost || e.cape_url == null || e.cape_url.isEmpty()) return base;
            String key = CapeStore.key(e.cape_url, e.cape_hash);
            ClientAsset.Texture tex = TEXTURES.get(key);
            if (tex == null) {
                if (BROKEN.containsKey(key)) return base;
                byte[] png = CapeStore.bytes(e.cape_url, e.cape_hash);
                if (png == null) return base;
                try {
                    NativeImage img = NativeImage.read(png);
                    ResourceLocation id = ResourceLocation.fromNamespaceAndPath("frost_client", "cape/" + key);
                    Minecraft.getInstance().getTextureManager().register(id, new DynamicTexture(() -> "frost_cape_" + key, img));
                    tex = new ClientAsset.ResourceTexture(id);
                    TEXTURES.put(key, tex);
                } catch (Exception ex) {
                    BROKEN.put(key, Boolean.TRUE);
                    return base;
                }
            }
            return new PlayerSkin(base.body(), tex, base.elytra(), base.model(), base.secure());
        } catch (Throwable t) {
            return base;
        }
    }
}
