package dev.frost.forge;

import dev.frost.core.Cps;
import dev.frost.core.FrostState;
import dev.frost.core.Models;
import net.minecraft.block.Block;
import net.minecraft.client.Minecraft;
import net.minecraft.client.entity.EntityPlayerSP;
import net.minecraft.client.network.NetworkPlayerInfo;
import net.minecraft.client.resources.I18n;
import net.minecraft.init.Blocks;
import net.minecraft.init.Items;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.potion.PotionEffect;
import net.minecraft.util.BlockPos;
import net.minecraft.util.MovingObjectPosition;
import org.lwjgl.input.Mouse;

import java.util.ArrayDeque;

/** Collects everything the HUD and the launcher need to know about the running game. */
public final class Stats {
    private static final Cps LEFT = new Cps();
    private static final Cps RIGHT = new Cps();
    private static boolean lastL;
    private static boolean lastR;
    private static volatile Float lastReach;

    private static final ArrayDeque<Long> PLACEMENTS = new ArrayDeque<Long>();
    private static Item lastItem;
    private static int lastCount;
    private static float best;

    public static volatile Models.Stats latest = new Models.Stats();

    private Stats() {}

    /** Once per rendered frame: frame-accurate click detection. */
    public static void frame(Minecraft mc) {
        if (mc.thePlayer == null || mc.currentScreen != null) {
            lastL = false;
            lastR = false;
            return;
        }
        boolean l = Mouse.isButtonDown(0);
        boolean r = Mouse.isButtonDown(1);
        if (l && !lastL) {
            LEFT.click();
            MovingObjectPosition mop = mc.objectMouseOver;
            if (mop != null && mop.typeOfHit == MovingObjectPosition.MovingObjectType.ENTITY && mop.hitVec != null) {
                lastReach = (float) mc.thePlayer.getPositionEyes(1.0F).distanceTo(mop.hitVec);
            }
        }
        if (r && !lastR) RIGHT.click();
        lastL = l;
        lastR = r;
    }

    static boolean isBridgeBlock(ItemStack st) {
        if (st == null) return false;
        Block b = Block.getBlockFromItem(st.getItem());
        return b == Blocks.wool || b == Blocks.stained_hardened_clay || b == Blocks.planks
            || b == Blocks.end_stone || b == Blocks.obsidian || b == Blocks.glass || b == Blocks.stained_glass;
    }

    public static void tick(Minecraft mc) {
        EntityPlayerSP p = mc.thePlayer;
        if (p == null || mc.theWorld == null) return;

        Models.Stats s = new Models.Stats();
        s.fps = Minecraft.getDebugFPS();
        s.cps_left = LEFT.count();
        s.cps_right = RIGHT.count();
        s.yaw = p.rotationYaw;
        s.pos = new double[] {p.posX, p.posY, p.posZ};
        s.last_reach = lastReach;

        s.keys.w = mc.gameSettings.keyBindForward.isKeyDown();
        s.keys.a = mc.gameSettings.keyBindLeft.isKeyDown();
        s.keys.s = mc.gameSettings.keyBindBack.isKeyDown();
        s.keys.d = mc.gameSettings.keyBindRight.isKeyDown();
        s.keys.space = mc.gameSettings.keyBindJump.isKeyDown();
        s.keys.lmb = mc.gameSettings.keyBindAttack.isKeyDown();
        s.keys.rmb = mc.gameSettings.keyBindUseItem.isKeyDown();

        if (mc.getNetHandler() != null) {
            NetworkPlayerInfo info = mc.getNetHandler().getPlayerInfo(p.getUniqueID());
            if (info != null) s.ping_ms = info.getResponseTime();
        }

        for (int i = 0; i < 4; i++) {
            ItemStack st = p.inventory.armorInventory[3 - i]; // 0 = head in the shared model
            if (st == null) continue;
            Models.Armor a = new Models.Armor();
            a.slot = i;
            a.name = st.getDisplayName();
            a.durability_pct = st.isItemStackDamageable()
                ? 1f - (float) st.getItemDamage() / Math.max(1, st.getMaxDamage())
                : 1f;
            s.armor.add(a);
        }

        for (Object o : p.getActivePotionEffects()) {
            PotionEffect e = (PotionEffect) o;
            Models.Potion po = new Models.Potion();
            po.name = I18n.format(e.getEffectName());
            po.amplifier = e.getAmplifier();
            po.remaining_secs = Math.max(0, e.getDuration() / 20);
            s.potions.add(po);
        }

        s.biome = mc.theWorld.getBiomeGenForCoords(new BlockPos(p)).biomeName;
        fillBedwars(p, s);
        latest = s;
    }

    private static void fillBedwars(EntityPlayerSP p, Models.Stats s) {
        ItemStack[] main = p.inventory.mainInventory;
        for (ItemStack st : main) {
            if (st == null) continue;
            Item it = st.getItem();
            if (it == Items.iron_ingot) s.bw_iron += st.stackSize;
            else if (it == Items.gold_ingot) s.bw_gold += st.stackSize;
            else if (it == Items.diamond) s.bw_diamond += st.stackSize;
            else if (it == Items.emerald) s.bw_emerald += st.stackSize;
            else if (isBridgeBlock(st)) s.bw_blocks += st.stackSize;
        }

        long now = System.nanoTime();
        ItemStack held = p.getHeldItem();
        if (FrostState.INSTANCE.bedwarsBridgeTrainer && isBridgeBlock(held)) {
            if (held.getItem() == lastItem && held.stackSize < lastCount) {
                int placed = Math.min(2, lastCount - held.stackSize);
                for (int k = 0; k < placed; k++) PLACEMENTS.addLast(now);
            }
            lastItem = held.getItem();
            lastCount = held.stackSize;
        } else {
            lastItem = null;
            lastCount = 0;
        }
        while (!PLACEMENTS.isEmpty() && now - PLACEMENTS.peekFirst() > 3000000000L) PLACEMENTS.removeFirst();
        float bps = 0f;
        if (PLACEMENTS.size() >= 2) {
            double span = (PLACEMENTS.peekLast() - PLACEMENTS.peekFirst()) / 1e9;
            if (span > 0.2) bps = (float) ((PLACEMENTS.size() - 1) / span);
        }
        if (PLACEMENTS.size() >= 5) best = Math.max(best, bps);
        s.bridge_bps = bps;
        s.bridge_best = best;
    }
}
