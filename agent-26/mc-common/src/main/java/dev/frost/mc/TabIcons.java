package dev.frost.mc;

import dev.frost.core.FrostState;
import com.google.gson.JsonObject;
import com.mojang.serialization.JsonOps;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.world.scores.PlayerTeam;

import java.util.ArrayList;
import java.util.List;

/**
 * Adds the snowflake in front of Frost users' names in the tab list. Uses the public
 * PlayerInfo display-name setter, so no mixin is needed. The launcher decides who is a
 * Frost user (cosmetics lookup); the local player always counts.
 */
public final class TabIcons {
    /** Private-use code point of the Frost icon font (assets/frost/font/icons.json). */
    private static final String ICON = "\uE000";
    private static final String FALLBACK = "❄";
    private static Component icon;

    private TabIcons() {}

    /** The pixel snowflake from our own font; falls back to a plain glyph if it cannot be built. */
    private static Component icon() {
        if (icon == null) {
            try {
                JsonObject o = new JsonObject();
                o.addProperty("text", ICON);
                o.addProperty("font", "frost:icons");
                icon = ComponentSerialization.CODEC.parse(JsonOps.INSTANCE, o).result().orElse(null);
            } catch (Throwable ignored) {
                icon = null;
            }
            if (icon == null) icon = Component.literal(FALLBACK).withStyle(ChatFormatting.AQUA);
        }
        return icon;
    }

    public static void tick(Minecraft mc, int ticks) {
        if (mc.getConnection() == null || mc.level == null || mc.player == null) return;
        if (ticks % 20 != 0) return; // once per second; server updates are re-covered on the next pass

        String self = noDashes(mc.player.getUUID().toString());
        List<String> uuids = new ArrayList<>();
        for (PlayerInfo info : mc.getConnection().getOnlinePlayers()) {
            String uuid = noDashes(info.getProfile().id().toString());
            String name = info.getProfile().name();
            uuids.add(uuid);

            boolean frost = uuid.equals(self) || FrostState.INSTANCE.isFrost(uuid);
            Component current = info.getTabListDisplayName();
            String shownText = current == null ? "" : current.getString();
            boolean hasFlake = shownText.startsWith(ICON) || shownText.startsWith(FALLBACK);

            if (frost && !hasFlake) {
                PlayerTeam team = mc.level.getScoreboard().getPlayersTeam(name);
                MutableComponent shown = PlayerTeam.formatNameForTeam(team, Component.literal(name));
                info.setTabListDisplayName(Component.empty().append(icon()).append(Component.literal(" ")).append(shown));
            } else if (!frost && hasFlake) {
                info.setTabListDisplayName(null);
            }
        }
        if (FrostMc.link() != null && FrostMc.link().connected() && ticks % 100 == 0) {
            FrostMc.sendTabPlayers(uuids);
        }
    }

    private static String noDashes(String uuid) {
        return uuid.replace("-", "");
    }
}
