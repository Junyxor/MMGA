//! Minecraft 1.21.1 MobCategory (`btq`) metadata; no spawning/world simulation.
use mmga_classfile::MemberRef;
use mmga_runtime::{Data,Rt,V};
const CLASS:&str="net/minecraft/world/entity/MobCategory";
const DESC:&str="Lnet/minecraft/world/entity/MobCategory;";
const DEFS:[(&str,&str,i32,bool,bool,i32);8]=[
    ("MONSTER","monster",70,false,false,128),("CREATURE","creature",10,true,true,128),
    ("AMBIENT","ambient",15,true,false,128),("AXOLOTLS","axolotls",5,true,false,128),
    ("UNDERGROUND_WATER_CREATURE","underground_water_creature",5,true,false,128),
    ("WATER_CREATURE","water_creature",5,true,false,128),("WATER_AMBIENT","water_ambient",20,true,false,64),
    ("MISC","misc",-1,true,true,128),
];
pub fn get_static_field(rt:&mut Rt,owner:&str,name:&str,desc:&str)->Option<V>{
    if owner!=CLASS||desc!=DESC{return None;}let(index,definition)=DEFS.iter().enumerate().find(|(_,def)|def.0==name)?;if let Some(value)=rt.get_static(owner,name){return Some(value);}
    let value=rt.new_instance(CLASS,0);let enum_name=rt.intern_str(name);let text=rt.intern_str(definition.1);
    for(field,v)in[("$name",enum_name),("$ordinal",V::I(index as i32)),("name",text),("max",V::I(definition.2)),("isFriendly",V::bool(definition.3)),("isPersistent",V::bool(definition.4)),("noDespawnDistance",V::I(32)),("despawnDistance",V::I(definition.5))]{rt.put_field(value,field,v);}rt.put_static(owner,name,value);Some(value)
}
pub fn call(rt:&mut Rt,member:&MemberRef,is_static:bool,args:&[V])->Option<V>{
    if member.owner!=CLASS{return None;}
    let(name,desc)=(member.name.as_str(),member.desc.as_str());
    if is_static{
        if name=="values"&&desc=="()[Lnet/minecraft/world/entity/MobCategory;"{let values=DEFS.iter().map(|def|get_static_field(rt,CLASS,def.0,DESC).unwrap()).collect();return Some(rt.alloc("[Lnet/minecraft/world/entity/MobCategory;",0,Data::Array(values)));}
        if name=="valueOf"&&desc=="(Ljava/lang/String;)Lnet/minecraft/world/entity/MobCategory;"{if args[0].is_null(){return Some(rt.throw_exception("java/lang/NullPointerException","Name is null"));}let name=rt.str_of(args[0])?.to_owned();return Some(get_static_field(rt,CLASS,&name,DESC).unwrap_or_else(||rt.throw_exception("java/lang/IllegalArgumentException",&format!("No enum constant MobCategory.{name}"))));}
        return None;
    }
    let field=match(name,desc){("getName"|"getSerializedName","()Ljava/lang/String;")=>"name",("getMaxInstancesPerChunk","()I")=>"max",("isFriendly","()Z")=>"isFriendly",("isPersistent","()Z")=>"isPersistent",("getDespawnDistance","()I")=>"despawnDistance",("getNoDespawnDistance","()I")=>"noDespawnDistance",_=>return None};Some(rt.get_field(args[0],field))
}
pub fn is_subtype(sub:&str,sup:&str)->bool{sub==CLASS&&matches!(sup,"java/lang/Enum"|"java/lang/Comparable"|"java/io/Serializable"|"net/minecraft/util/StringRepresentable")}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn categories_keep_actual_caps_descriptive_names_flags_and_despawn_ranges(){
        let mut rt=Rt::new();let mut values=Vec::new();
        for(index,(name,serialized,cap,friendly,persistent,distance))in DEFS.into_iter().enumerate(){
            let value=get_static_field(&mut rt,CLASS,name,DESC).expect("category must carry actual metadata");values.push(value);assert_eq!(get_static_field(&mut rt,CLASS,name,DESC),Some(value));
            assert_eq!(rt.get_field(value,"$ordinal"),V::I(index as i32));
            let text=call(&mut rt,&MemberRef::new(CLASS,"getSerializedName","()Ljava/lang/String;"),false,&[value]).unwrap();assert_eq!(rt.str_of(text),Some(serialized));
            for(method,desc,expected)in[("getMaxInstancesPerChunk","()I",V::I(cap)),("isFriendly","()Z",V::bool(friendly)),("isPersistent","()Z",V::bool(persistent)),("getDespawnDistance","()I",V::I(distance)),("getNoDespawnDistance","()I",V::I(32))]{assert_eq!(call(&mut rt,&MemberRef::new(CLASS,method,desc),false,&[value]),Some(expected));}
        }
        let array=call(&mut rt,&MemberRef::new(CLASS,"values","()[Lnet/minecraft/world/entity/MobCategory;"),true,&[]).unwrap();assert!(matches!(&rt.get_obj(array).unwrap().data,Data::Array(xs)if xs==&values));let another=call(&mut rt,&MemberRef::new(CLASS,"values","()[Lnet/minecraft/world/entity/MobCategory;"),true,&[]).unwrap();assert_ne!(array,another);
        assert!(get_static_field(&mut rt,CLASS,"CODEC","Lcom/mojang/serialization/Codec;").is_none());
    }
}
