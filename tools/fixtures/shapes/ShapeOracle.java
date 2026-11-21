// Original behavior oracle. Runs against the operator's own Minecraft JAR;
// this file contains no copied game implementation or compile-time game stubs.
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.core.Direction.Axis;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.shapes.*;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Rotation;

public final class ShapeOracle {
    static String bits(double value) { return String.format("%016x",Double.doubleToRawLongBits(value)); }
    static List<String> coords(AABB box) { return Arrays.asList(bits(box.minX),bits(box.minY),bits(box.minZ),bits(box.maxX),bits(box.maxY),bits(box.maxZ)); }
    interface Factory { VoxelShape get(); }
    static JsonObject shape(Factory factory) {
        JsonObject out=new JsonObject();
        try {
            VoxelShape value=factory.get();
            out.addProperty("class",value.getClass().getSimpleName());
            out.addProperty("empty",value.isEmpty());
            JsonArray boxes=new JsonArray(); for(AABB box:value.toAabbs()) boxes.add(new Gson().toJsonTree(coords(box))); out.add("boxes",boxes);
            JsonArray min=new JsonArray(),max=new JsonArray(); for(Axis axis:Axis.values()){min.add(bits(value.min(axis)));max.add(bits(value.max(axis)));}out.add("min",min);out.add("max",max);
            try { out.add("bounds",new Gson().toJsonTree(coords(value.bounds()))); } catch(Throwable e){out.addProperty("bounds_error",e.getClass().getSimpleName()+": "+e.getMessage());}
        } catch(Throwable e) {out.addProperty("error",e.getClass().getSimpleName()+": "+e.getMessage());}
        return out;
    }
    public static void main(String[] args)throws Exception {
        JsonObject out=new JsonObject();
        net.minecraft.SharedConstants.tryDetectVersion();
        String minecraft=net.minecraft.SharedConstants.getCurrentVersion().getName();
        if(!minecraft.equals("1.21.1")) throw new IllegalStateException("Expected Minecraft 1.21.1, got "+minecraft);
        out.addProperty("minecraft",minecraft);
        double[][] cases={
            {0,0,0,16,16,16},{1,2,3,15,14,13},{-16,-8,0,32,24,16},{4,4,4,4,12,12},
            {12,0,0,4,16,16},{0,0,0,1e-8,1,1},{0,0,0,1e-7,1,1},
            {1e-8,0,0,1,1,1},{1e-7,0,0,1,1,1},{-1e-8,0,0,1+1e-8,1,1},
            {0,0,0,Double.NaN,1,1},{Double.NaN,0,0,1,1,1},
            {Double.NEGATIVE_INFINITY,0,0,Double.POSITIVE_INFINITY,1,1},
            {Double.POSITIVE_INFINITY,0,0,Double.POSITIVE_INFINITY,1,1},
            {-0.0,0,0,1,1,1},{0,0,0,0.50000001,0.75,0.125},
        };
        JsonArray inputs=new JsonArray(); for(int i=0;i<cases.length;i++){final double[] c=cases[i];JsonObject item=new JsonObject();JsonArray raw=new JsonArray();for(double d:c)raw.add(bits(d));item.add("input",raw);item.add("create",shape(()->Shapes.create(c[0],c[1],c[2],c[3],c[4],c[5])));item.add("box",shape(()->Shapes.box(c[0],c[1],c[2],c[3],c[4],c[5])));item.add("block_box",shape(()->Block.box(c[0],c[1],c[2],c[3],c[4],c[5])));inputs.add(item);}out.add("cases",inputs);
        VoxelShape[] values={Shapes.empty(),Shapes.block(),Shapes.box(0,0,0,.5,1,1),Shapes.box(.5,0,0,1,1,1),Shapes.box(.25,.25,.25,.75,.75,.75),Shapes.box(.5,1e-8,0,1,1,1),Shapes.box(.5000001,0,0,1,1,1),Shapes.box(-1,-1,-1,2,2,2)};
        JsonArray joins=new JsonArray();String[] names={"FALSE","NOT_OR","ONLY_SECOND","NOT_FIRST","ONLY_FIRST","NOT_SECOND","NOT_SAME","NOT_AND","AND","SAME","SECOND","CAUSES","FIRST","CAUSED_BY","OR","TRUE"};BooleanOp[] ops={BooleanOp.FALSE,BooleanOp.NOT_OR,BooleanOp.ONLY_SECOND,BooleanOp.NOT_FIRST,BooleanOp.ONLY_FIRST,BooleanOp.NOT_SECOND,BooleanOp.NOT_SAME,BooleanOp.NOT_AND,BooleanOp.AND,BooleanOp.SAME,BooleanOp.SECOND,BooleanOp.CAUSES,BooleanOp.FIRST,BooleanOp.CAUSED_BY,BooleanOp.OR,BooleanOp.TRUE};
        for(int a=0;a<values.length;a++)for(int b=0;b<values.length;b++)for(int k=0;k<ops.length;k++){final int x=a,y=b;final BooleanOp op=ops[k];JsonObject item=new JsonObject();item.addProperty("a",a);item.addProperty("b",b);item.addProperty("op",names[k]);item.add("shape",shape(()->Shapes.join(values[x],values[y],op)));try{item.addProperty("not_empty",Shapes.joinIsNotEmpty(values[a],values[b],op));}catch(Throwable e){item.addProperty("error",e.getClass().getSimpleName());}joins.add(item);}out.add("joins",joins);
        JsonArray collisions=new JsonArray(); double[][] boxes={{-1,.25,.25,0,.75,.75},{-1,.25,.25,-.25,.75,.75},{1,.25,.25,2,.75,.75},{.25,.25,.25,.75,.75,.75},{-1,1,0,0,2,1},{-1,1-1e-8,0,0,2,1},{-1,1-2e-7,0,0,2,1}};
        for(int s=0;s<values.length;s++)for(int a=0;a<3;a++)for(int i=0;i<boxes.length;i++)for(double distance:new double[]{-2,-.25,-1e-8,0,1e-8,.25,2}){double[] b=boxes[i];JsonObject item=new JsonObject();item.addProperty("shape",s);item.addProperty("axis",a);item.addProperty("box",i);item.addProperty("distance",bits(distance));item.addProperty("result",bits(values[s].collide(Axis.values()[a],new AABB(b[0],b[1],b[2],b[3],b[4],b[5]),distance)));collisions.add(item);}out.add("collisions",collisions);
        JsonArray moves=new JsonArray();for(int i=0;i<values.length;i++){final int s=i;JsonObject item=new JsonObject();item.addProperty("shape",s);item.add("moved",shape(()->values[s].move(-.5,2,.125)));item.add("optimized",shape(()->values[s].optimize()));moves.add(item);}out.add("moves",moves);
        VoxelShape left=values[2],right=values[3],empty=Shapes.empty(),block=Shapes.block();
        VoxelShape full=Shapes.joinUnoptimized(left,right,BooleanOp.OR),disjoint=Shapes.joinUnoptimized(left,right,BooleanOp.AND);
        JsonObject identities=new JsonObject();
        identities.addProperty("create_is_block",Shapes.create(0,0,0,1,1,1)==block);
        identities.addProperty("self_or_is_self",Shapes.joinUnoptimized(left,left,BooleanOp.OR)==left);
        identities.addProperty("or_empty_is_self",Shapes.joinUnoptimized(left,empty,BooleanOp.OR)==left);
        identities.addProperty("empty_or_is_self",Shapes.joinUnoptimized(empty,left,BooleanOp.OR)==left);
        identities.addProperty("joined_full_is_block",full==block);
        identities.addProperty("optimized_full_is_block",full.optimize()==block);
        identities.addProperty("joined_empty_is_empty",disjoint.isEmpty());
        identities.addProperty("joined_empty_is_canonical",disjoint==empty);
        identities.addProperty("empty_varargs_is_self",Shapes.or(left,new VoxelShape[0])==left);
        out.add("identities",identities);
        JsonArray rotations = new JsonArray();
        for (Rotation rotation : Rotation.values()) {
            JsonObject item = new JsonObject();
            item.addProperty("name", rotation.name());
            item.addProperty("id", rotation.getSerializedName());
            JsonArray composition = new JsonArray();
            for (Rotation other : Rotation.values()) composition.add(rotation.getRotated(other).name());
            item.add("composition", composition);
            JsonArray directions = new JsonArray();
            for (net.minecraft.core.Direction direction : net.minecraft.core.Direction.values()) directions.add(rotation.rotate(direction).name());
            item.add("directions", directions);
            JsonArray integers = new JsonArray();
            for (int value : new int[]{Integer.MIN_VALUE, -99, -7, 0, 5, 99, Integer.MAX_VALUE}) {
                for (int size : new int[]{Integer.MIN_VALUE, -16, -1, 0, 6, 16, Integer.MAX_VALUE}) {
                    JsonObject integer = new JsonObject();
                    integer.addProperty("value", value);
                    integer.addProperty("size", size);
                    try { integer.addProperty("result", rotation.rotate(value, size)); }
                    catch (Throwable error) { integer.addProperty("error", error.getClass().getSimpleName()); }
                    integers.add(integer);
                }
            }
            item.add("integers", integers);
            rotations.add(item);
        }
        out.add("rotations", rotations);
        JsonArray directionValues = new JsonArray();
        for (net.minecraft.core.Direction direction : net.minecraft.core.Direction.values()) {
            JsonObject item = new JsonObject();
            item.addProperty("name", direction.name());
            item.addProperty("opposite", direction.getOpposite().name());
            item.addProperty("axis", direction.getAxis().name());
            item.addProperty("axis_direction", direction.getAxisDirection().name());
            item.add("steps", new Gson().toJsonTree(new int[]{direction.getStepX(), direction.getStepY(), direction.getStepZ()}));
            try { item.addProperty("clockwise", direction.getClockWise().name()); }
            catch (Throwable error) { item.addProperty("clockwise_error", error.getClass().getSimpleName() + ": " + error.getMessage()); }
            try { item.addProperty("counterclockwise", direction.getCounterClockWise().name()); }
            catch (Throwable error) { item.addProperty("counterclockwise_error", error.getClass().getSimpleName() + ": " + error.getMessage()); }
            JsonArray clockwise = new JsonArray(), counterclockwise = new JsonArray();
            for (Axis axis : Axis.values()) {
                clockwise.add(direction.getClockWise(axis).name());
                counterclockwise.add(direction.getCounterClockWise(axis).name());
            }
            item.add("axis_clockwise", clockwise);
            item.add("axis_counterclockwise", counterclockwise);
            directionValues.add(item);
        }
        out.add("directions", directionValues);
        Files.writeString(Path.of(args[0]),new GsonBuilder().setPrettyPrinting().create().toJson(out));
        System.out.println("PASS actual 1.21.1 oracle: "+cases.length+" box cases, "+joins.size()+" joins, "+collisions.size()+" collisions");
    }
}
