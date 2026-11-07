package dev.frost.mc;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.minecraft.resources.Identifier;

/** Minecraft 1.21.11 entry point (ResourceLocation was renamed to Identifier). */
public class Entry implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        FrostMc.init("1.21.11");
        HudElementRegistry.attachElementBefore(
            VanillaHudElements.CHAT, Identifier.fromNamespaceAndPath("frost_client", "hud"), FrostHud::render);
        HudElementRegistry.replaceElement(
            VanillaHudElements.CROSSHAIR, orig -> (g, dt) -> FrostHud.crosshair(g, dt, orig));
    }
}
