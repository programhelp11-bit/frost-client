package dev.frost.mc;

import dev.frost.core.FrostState;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.network.chat.Component;
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
    private static final String FLAKE = "❄";

    private TabIcons() {}

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
            boolean hasFlake = current != null && current.getString().startsWith(FLAKE);

            if (frost && !hasFlake) {
                PlayerTeam team = mc.level.getScoreboard().getPlayersTeam(name);
                MutableComponent shown = PlayerTeam.formatNameForTeam(team, Component.literal(name));
                info.setTabListDisplayName(Component.literal(FLAKE + " ").withStyle(ChatFormatting.AQUA).append(shown));
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
