// TC-0036 A and B: caller fixtures, never production dispatch.
const column=(kind,optional=false)=>({kind,optional});
const table=(columns,relations={})=>({columns,relations});
const one=(target,key)=>({kind:'ToOne',table:target,foreign_key:key,optional:false});
export const schemas=[
 {version:'1',tables:{authors:table({name:column('String')}),articles:table({title:column('String'),score:column('Integer'),ratio:column('Float'),active:column('Boolean'),at:column('Timestamp'),note:column('String',true)},{author:one('authors','author_id'),tags:{kind:'ManyToMany',table:'tags',through:'article_tags',source_relation:'article',target_relation:'tag'}}),tags:table({label:column('String')}),article_tags:table({},{article:one('articles','article_id'),tag:one('tags','tag_id')})}},
 {version:'1',tables:{sensors:table({label:column('String')},{readings:{kind:'HasMany',table:'readings',foreign_key:'sensor_id'}}),readings:table({value:column('Float'),at:column('Timestamp')},{sensor:one('sensors','sensor_id')})}}
];
