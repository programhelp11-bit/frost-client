package dev.frost.mc;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.minecraft.resources.ResourceLocation;

/** Minecraft 1.21.10 entry point (Mojang name for identifiers: ResourceLocation). */
public class Entry implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        FrostMc.init("1.21.10");
        HudElementRegistry.attachElementBefore(
            VanillaHudElements.CHAT, ResourceLocation.fromNamespaceAndPath("frost_client", "hud"), FrostHud::render);
    }
}
