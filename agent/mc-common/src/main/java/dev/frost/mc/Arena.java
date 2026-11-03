package dev.frost.mc;

import dev.frost.core.ArenaPlan;
import dev.frost.core.Models;
import net.minecraft.client.Minecraft;
import net.minecraft.core.BlockPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

import java.util.List;
import java.util.UUID;

/**
 * Builds a practice arena around the player and hands out a kit. Works through the integrated
 * server, so it only does something in singleplayer (the server never sees client-only bots).
 */
public final class Arena {
    /** Absolute bot spawn of the last arena, or null when none was built. */
    static volatile double[] botSpawn;

    private Arena() {}

    public static void setup(Minecraft mc, Models.BotConfig cfg) {
        botSpawn = null;
        boolean wantArena = ArenaPlan.isArena(cfg.arena);
        if (!wantArena && !cfg.kit) return;
        MinecraftServer server = mc.getSingleplayerServer();
        if (server == null || mc.player == null) return; // Bots already refuses multiplayer
        final UUID id = mc.player.getUUID();
        final int cx = (int) Math.floor(mc.player.getX());
        final int cz = (int) Math.floor(mc.player.getZ());
        final int floorY = (int) Math.floor(mc.player.getY()) - 1;
        final String kind = cfg.arena;
        final String style = cfg.style;
        final boolean kit = cfg.kit;

        if (wantArena) {
            int[] b = ArenaPlan.botSpot(kind);
            botSpawn = new double[] {cx + 0.5 + b[0], floorY + b[1], cz + 0.5 + b[2]};
        }

        server.execute(() -> {
            ServerPlayer sp = server.getPlayerList().getPlayer(id);
            if (sp == null) return;
            if (wantArena) {
                ServerLevel level = (ServerLevel) sp.level();
                List<ArenaPlan.Cell> cells = ArenaPlan.build(kind);
                for (ArenaPlan.Cell c : cells) {
                    level.setBlock(new BlockPos(cx + c.dx, floorY + c.dy, cz + c.dz), state(c.mat), 2);
                }
                int[] p = ArenaPlan.playerSpot(kind);
                sp.connection.teleport(cx + 0.5 + p[0], floorY + p[1], cz + 0.5 + p[2], 0f, 0f);
            }
            if (kit) giveKit(sp, style);
            sp.setHealth(sp.getMaxHealth());
            sp.getFoodData().setFoodLevel(20);
        });
    }

    private static BlockState state(ArenaPlan.Mat m) {
        Block b;
        switch (m) {
            case FLOOR_A -> b = Blocks.LIGHT_BLUE_CONCRETE;
            case FLOOR_B -> b = Blocks.WHITE_CONCRETE;
            case WALL -> b = Blocks.PACKED_ICE;
            case PILLAR, OBSIDIAN -> b = Blocks.OBSIDIAN;
            case END_STONE -> b = Blocks.END_STONE;
            default -> b = Blocks.AIR;
        }
        return b.defaultBlockState();
    }

    private static void giveKit(ServerPlayer sp, String style) {
        sp.getInventory().clearContent();
        boolean bw = "bedwars_rush".equals(style);
        sp.setItemSlot(EquipmentSlot.HEAD, new ItemStack(bw ? Items.IRON_HELMET : Items.NETHERITE_HELMET));
        sp.setItemSlot(EquipmentSlot.CHEST, new ItemStack(bw ? Items.IRON_CHESTPLATE : Items.NETHERITE_CHESTPLATE));
        sp.setItemSlot(EquipmentSlot.LEGS, new ItemStack(bw ? Items.IRON_LEGGINGS : Items.NETHERITE_LEGGINGS));
        sp.setItemSlot(EquipmentSlot.FEET, new ItemStack(bw ? Items.IRON_BOOTS : Items.NETHERITE_BOOTS));
        ItemStack[] hotbar;
        switch (style) {
            case "crystal" -> {
                hotbar = new ItemStack[] {new ItemStack(Items.NETHERITE_SWORD), new ItemStack(Items.OBSIDIAN, 64),
                    new ItemStack(Items.END_CRYSTAL, 64), new ItemStack(Items.GOLDEN_APPLE, 16),
                    new ItemStack(Items.ENDER_PEARL, 16), new ItemStack(Items.COOKED_BEEF, 64)};
                sp.setItemSlot(EquipmentSlot.OFFHAND, new ItemStack(Items.TOTEM_OF_UNDYING));
            }
            case "axe_shield" -> {
                hotbar = new ItemStack[] {new ItemStack(Items.NETHERITE_AXE), new ItemStack(Items.NETHERITE_SWORD),
                    new ItemStack(Items.BOW), new ItemStack(Items.GOLDEN_APPLE, 16),
                    new ItemStack(Items.ENDER_PEARL, 16), new ItemStack(Items.COBBLESTONE, 64), new ItemStack(Items.ARROW, 64)};
                sp.setItemSlot(EquipmentSlot.OFFHAND, new ItemStack(Items.SHIELD));
            }
            case "mace" -> {
                hotbar = new ItemStack[] {new ItemStack(Items.MACE), new ItemStack(Items.WIND_CHARGE, 64),
                    new ItemStack(Items.GOLDEN_APPLE, 16), new ItemStack(Items.ENDER_PEARL, 16),
                    new ItemStack(Items.COBBLESTONE, 64), new ItemStack(Items.WATER_BUCKET)};
                sp.setItemSlot(EquipmentSlot.OFFHAND, new ItemStack(Items.SHIELD));
            }
            case "bedwars_rush" -> hotbar = new ItemStack[] {new ItemStack(Items.IRON_SWORD),
                new ItemStack(Items.WHITE_WOOL, 64), new ItemStack(Items.WHITE_WOOL, 64), new ItemStack(Items.SHEARS),
                new ItemStack(Items.IRON_PICKAXE), new ItemStack(Items.GOLDEN_APPLE, 8), new ItemStack(Items.ENDER_PEARL, 2)};
            default -> {
                hotbar = new ItemStack[] {new ItemStack(Items.NETHERITE_SWORD), new ItemStack(Items.BOW),
                    new ItemStack(Items.GOLDEN_APPLE, 16), new ItemStack(Items.ENDER_PEARL, 16),
                    new ItemStack(Items.WATER_BUCKET), new ItemStack(Items.COBBLESTONE, 64),
                    new ItemStack(Items.COOKED_BEEF, 64), new ItemStack(Items.ARROW, 64)};
                sp.setItemSlot(EquipmentSlot.OFFHAND, new ItemStack(Items.SHIELD));
            }
        }
        for (int i = 0; i < hotbar.length && i < 9; i++) sp.getInventory().setItem(i, hotbar[i]);
    }
}
