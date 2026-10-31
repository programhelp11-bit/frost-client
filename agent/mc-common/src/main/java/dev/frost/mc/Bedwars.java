package dev.frost.mc;

import dev.frost.core.FrostState;
import dev.frost.core.Models;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.tags.ItemTags;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

import java.util.ArrayDeque;

/** Bedwars helpers: resource / block counters and the bridging speed trainer. */
public final class Bedwars {
    private static final ArrayDeque<Long> PLACEMENTS = new ArrayDeque<>();
    private static Item lastItem;
    private static int lastCount;
    private static float best;

    private Bedwars() {}

    public static boolean isBridgeBlock(ItemStack st) {
        return !st.isEmpty()
            && (st.is(ItemTags.WOOL) || st.is(ItemTags.TERRACOTTA) || st.is(ItemTags.PLANKS)
                || st.is(Items.END_STONE) || st.is(Items.OBSIDIAN) || st.is(Items.GLASS));
    }

    public static void fill(Minecraft mc, Models.Stats s) {
        LocalPlayer p = mc.player;
        if (p == null) return;
        Inventory inv = p.getInventory();
        for (int i = 0; i < inv.getContainerSize(); i++) {
            ItemStack st = inv.getItem(i);
            if (st.isEmpty()) continue;
            if (st.is(Items.IRON_INGOT)) s.bw_iron += st.getCount();
            else if (st.is(Items.GOLD_INGOT)) s.bw_gold += st.getCount();
            else if (st.is(Items.DIAMOND)) s.bw_diamond += st.getCount();
            else if (st.is(Items.EMERALD)) s.bw_emerald += st.getCount();
            else if (isBridgeBlock(st)) s.bw_blocks += st.getCount();
        }

        long now = System.nanoTime();
        ItemStack held = p.getMainHandItem();
        if (FrostState.INSTANCE.bedwarsBridgeTrainer && isBridgeBlock(held)) {
            if (held.getItem() == lastItem && held.getCount() < lastCount) {
                int placed = Math.min(2, lastCount - held.getCount());
                for (int k = 0; k < placed; k++) PLACEMENTS.addLast(now);
            }
            lastItem = held.getItem();
            lastCount = held.getCount();
        } else {
            lastItem = null;
            lastCount = 0;
        }
        while (!PLACEMENTS.isEmpty() && now - PLACEMENTS.peekFirst() > 3_000_000_000L) PLACEMENTS.removeFirst();
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
