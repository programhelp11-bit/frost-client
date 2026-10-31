package dev.frost.forge;

import dev.frost.core.FrostState;
import net.minecraft.client.Minecraft;
import net.minecraft.client.network.NetworkPlayerInfo;
import net.minecraft.scoreboard.ScorePlayerTeam;
import net.minecraft.util.ChatComponentText;
import net.minecraft.util.EnumChatFormatting;
import net.minecraft.util.IChatComponent;

import java.util.ArrayList;
import java.util.List;

/** Snowflake in front of Frost users' names in the tab list (display-name setter, no coremod). */
public final class TabIcons {
    private static final String FLAKE = "❄";

    private TabIcons() {}

    public static void tick(Minecraft mc, int ticks) {
        if (mc.getNetHandler() == null || mc.theWorld == null || mc.thePlayer == null) return;
        if (ticks % 20 != 0) return;

        String self = mc.thePlayer.getUniqueID().toString().replace("-", "");
        List<String> uuids = new ArrayList<String>();
        for (NetworkPlayerInfo info : mc.getNetHandler().getPlayerInfoMap()) {
            String uuid = info.getGameProfile().getId().toString().replace("-", "");
            String name = info.getGameProfile().getName();
            uuids.add(uuid);

            boolean frost = uuid.equals(self) || FrostState.INSTANCE.isFrost(uuid);
            IChatComponent current = info.getDisplayName();
            boolean hasFlake = current != null && current.getUnformattedText().startsWith(FLAKE);

            if (frost && !hasFlake) {
                String shown = ScorePlayerTeam.formatPlayerName(mc.theWorld.getScoreboard().getPlayersTeam(name), name);
                info.setDisplayName(new ChatComponentText(EnumChatFormatting.AQUA + FLAKE + " " + EnumChatFormatting.RESET + shown));
            } else if (!frost && hasFlake) {
                info.setDisplayName(null);
            }
        }
        if (FrostForge.link() != null && FrostForge.link().connected() && ticks % 100 == 0) {
            FrostForge.sendTabPlayers(uuids);
        }
    }
}
