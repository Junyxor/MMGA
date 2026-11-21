//! NeoForge 1.21.1 RecipeType.simple snapshots ResourceLocation.toString at creation.
//! Recipe evaluation is not implemented by this metadata factory.
use mmga_classfile::{op,MemberRef};
use mmga_runtime::{invoke_member,ExecCtx,V};
const TYPE:&str="net/minecraft/world/item/crafting/RecipeType";
const SIMPLE:&str="mmga/internal/SimpleRecipeType";
pub fn is_subtype(sub:&str,sup:&str)->bool{sub==SIMPLE&&sup==TYPE}
pub fn call(ctx:&mut ExecCtx<'_>,member:&MemberRef,is_static:bool,args:&[V])->Option<V>{
    if member.owner==TYPE&&is_static&&member.name=="simple"&&member.desc=="(Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/world/item/crafting/RecipeType;"{
        let text=invoke_member(ctx,op::INVOKEVIRTUAL,&MemberRef::new("net/minecraft/resources/ResourceLocation","toString","()Ljava/lang/String;"),&[*args.first()?]);if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}let value=ctx.rt.new_instance(SIMPLE,0);ctx.rt.put_field(value,"$description",text);return Some(value);
    }
    if !is_static&&member.name=="toString"&&member.desc=="()Ljava/lang/String;"&&ctx.rt.class_of(*args.first()?)==Some(SIMPLE){return Some(ctx.rt.get_field(args[0],"$description"));}
    None
}
#[cfg(test)]
mod tests{
    use super::*;
    use mmga_runtime::Rt;
    #[test]
    fn recipe_type_factory_snapshots_names_and_does_not_register_or_alias_objects(){
        let mut prepared=crate::PreparedLoad::from_jars(vec![],true).unwrap();let mut rt=Rt::new();let id=rt.new_pinned_handle("net/minecraft/resources/ResourceLocation","resource_location",0,"ae2:transform");
        let mut ctx=ExecCtx{rt:&mut rt,linked:&prepared.linked,host:&mut prepared.loader,depth:0};
        let method=MemberRef::new(TYPE,"simple","(Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/world/item/crafting/RecipeType;");let first=invoke_member(&mut ctx,op::INVOKESTATIC,&method,&[id]);assert!(ctx.rt.pending_exception.is_none());let second=invoke_member(&mut ctx,op::INVOKESTATIC,&method,&[id]);assert_ne!(first,second);assert!(ctx.linked.is_subtype(ctx.rt.class_of(first).unwrap(),TYPE,ctx.host));let text=invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new("java/lang/Object","toString","()Ljava/lang/String;"),&[first]);assert_eq!(ctx.rt.str_of(text),Some("ae2:transform"));
        invoke_member(&mut ctx,op::INVOKESTATIC,&method,&[V::NULL]);let error=ctx.rt.take_exception().unwrap();assert!(ctx.rt.exception_summary(error).contains("NullPointerException"));
        assert_eq!(prepared.loader.state.registries.total_mod_entries(),0);
    }
}
