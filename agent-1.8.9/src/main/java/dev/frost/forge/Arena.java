package dev.frost.forge;

import dev.frost.core.ArenaPlan;
import dev.frost.core.Models;
import net.minecraft.client.Minecraft;
import net.minecraft.entity.player.EntityPlayerMP;
import net.minecraft.init.Blocks;
import net.minecraft.init.Items;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.block.Block;
import net.minecraft.server.integrated.IntegratedServer;
import net.minecraft.util.BlockPos;
import net.minecraft.world.World;

import java.util.List;
import java.util.UUID;

/** Builds a practice arena and kit through the integrated server (singleplayer only). */
public final class Arena {
    /** Absolute bot spawn of the last arena, or null. */
    static volatile double[] botSpawn;

    private Arena() {}

    public static void setup(Minecraft mc, final Models.BotConfig cfg) {
        botSpawn = null;
        final boolean wantArena = ArenaPlan.isArena(cfg.arena);
        if (!wantArena && !cfg.kit) return;
        final IntegratedServer server = mc.getIntegratedServer();
        if (server == null || mc.thePlayer == null) return;
        final UUID id = mc.thePlayer.getUniqueID();
        final int cx = (int) Math.floor(mc.thePlayer.posX);
        final int cz = (int) Math.floor(mc.thePlayer.posZ);
        final int floorY = (int) Math.floor(mc.thePlayer.posY) - 1;
        if (wantArena) {
            int[] b = ArenaPlan.botSpot(cfg.arena);
            botSpawn = new double[] {cx + 0.5 + b[0], floorY + b[1], cz + 0.5 + b[2]};
        }
        server.addScheduledTask(new Runnable() {
            public void run() {
                EntityPlayerMP sp = server.getConfigurationManager().getPlayerByUUID(id);
                if (sp == null) return;
                if (wantArena) {
                    World world = sp.worldObj;
                    List<ArenaPlan.Cell> cells = ArenaPlan.build(cfg.arena);
                    for (ArenaPlan.Cell c : cells) {
                        world.setBlockState(new BlockPos(cx + c.dx, floorY + c.dy, cz + c.dz), block(c.mat).getDefaultState(), 2);
                    }
                    int[] p = ArenaPlan.playerSpot(cfg.arena);
                    sp.playerNetServerHandler.setPlayerLocation(cx + 0.5 + p[0], floorY + p[1], cz + 0.5 + p[2], 0f, 0f);
                }
                if (cfg.kit) giveKit(sp, cfg.style);
                sp.setHealth(sp.getMaxHealth());
                sp.getFoodStats().setFoodLevel(20);
            }
        });
    }

    private static Block block(ArenaPlan.Mat m) {
        switch (m) {
            case FLOOR_A: return Blocks.lapis_block;
            case FLOOR_B: return Blocks.quartz_block;
            case WALL: return Blocks.packed_ice;
            case PILLAR:
            case OBSIDIAN: return Blocks.obsidian;
            case END_STONE: return Blocks.end_stone;
            default: return Blocks.air;
        }
    }

    private static void giveKit(EntityPlayerMP sp, String style) {
        for (int i = 0; i < sp.inventory.mainInventory.length; i++) sp.inventory.mainInventory[i] = null;
        boolean bw = "bedwars_rush".equals(style);
        sp.inventory.armorInventory[3] = new ItemStack(bw ? Items.iron_helmet : Items.diamond_helmet);
        sp.inventory.armorInventory[2] = new ItemStack(bw ? Items.iron_chestplate : Items.diamond_chestplate);
        sp.inventory.armorInventory[1] = new ItemStack(bw ? Items.iron_leggings : Items.diamond_leggings);
        sp.inventory.armorInventory[0] = new ItemStack(bw ? Items.iron_boots : Items.diamond_boots);
        Item wool = Item.getItemFromBlock(Blocks.wool);
        ItemStack[] hotbar;
        if (bw) {
            hotbar = new ItemStack[] {new ItemStack(Items.iron_sword), new ItemStack(wool, 64), new ItemStack(wool, 64),
                new ItemStack(Items.shears), new ItemStack(Items.iron_pickaxe), new ItemStack(Items.golden_apple, 8),
                new ItemStack(Items.ender_pearl, 2)};
        } else if ("axe_shield".equals(style)) {
            hotbar = new ItemStack[] {new ItemStack(Items.diamond_axe), new ItemStack(Items.diamond_sword),
                new ItemStack(Items.bow), new ItemStack(Items.golden_apple, 16), new ItemStack(Items.ender_pearl, 16),
                new ItemStack(Blocks.cobblestone, 64), new ItemStack(Items.arrow, 64)};
        } else {
            hotbar = new ItemStack[] {new ItemStack(Items.diamond_sword), new ItemStack(Items.bow),
                new ItemStack(Items.golden_apple, 16), new ItemStack(Items.ender_pearl, 16),
                new ItemStack(Items.water_bucket), new ItemStack(Blocks.cobblestone, 64),
                new ItemStack(Items.cooked_beef, 64), new ItemStack(Items.arrow, 64)};
        }
        for (int i = 0; i < hotbar.length && i < 9; i++) sp.inventory.mainInventory[i] = hotbar[i];
        sp.inventoryContainer.detectAndSendChanges();
    }
}
