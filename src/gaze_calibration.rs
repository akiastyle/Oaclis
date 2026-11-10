// gaze_calibration.rs v1.0.0
//! Classical dark-region pupil measurement and a calibrated screen-coordinate fit.
use serde::Deserialize;
use serde_json::{Value,json};
use std::io;

#[derive(Deserialize)]
struct Eye { width: usize, height: usize, pixels: Vec<u8> }

fn pupil(eye:&Eye)->io::Result<[f64;2]> {
    let (w,h)=(eye.width,eye.height);
    if w!=80 || h!=40 || eye.pixels.len()!=w*h {return Err(crate::invalid("Formato occhio non valido"));}
    let mut pixels=vec![255u8;w*h];
    for y in 1..h-1 { for x in 1..w-1 {
        let sum:u32=(y-1..=y+1).flat_map(|yy|(x-1..=x+1).map(move|xx|u32::from(eye.pixels[yy*w+xx]))).sum();
        pixels[y*w+x]=(sum/9) as u8;
    }}
    let mut sorted:Vec<u8>=(2..h-2).flat_map(|y|(2..w-2).map(move|x|y*w+x)).map(|i|pixels[i]).collect();
    sorted.sort_unstable();
    let dark=sorted[sorted.len()/50];let median=sorted[sorted.len()/2];
    if median.saturating_sub(dark)<15 {return Err(crate::invalid("Pupilla non distinguibile"));}
    let threshold=dark.saturating_add((median-dark)/4);
    let mut seen=vec![false;w*h];let mut best=None;let mut score=0.;
    for seed in 0..w*h {
        if seen[seed] || pixels[seed]>threshold {continue;}
        let mut queue=vec![seed];seen[seed]=true;
        let (mut area,mut sx,mut sy,mut minx,mut maxx,mut miny,mut maxy)=(0usize,0.,0.,w,0,h,0);
        while let Some(i)=queue.pop(){
            let (x,y)=(i%w,i/w);area+=1;sx+=x as f64;sy+=y as f64;
            minx=minx.min(x);maxx=maxx.max(x);miny=miny.min(y);maxy=maxy.max(y);
            for next in [i.checked_sub(1).filter(|_|x>0),i.checked_add(1).filter(|_|x+1<w),i.checked_sub(w),i.checked_add(w).filter(|&j|j<w*h)].into_iter().flatten(){
                if !seen[next] && pixels[next]<=threshold {seen[next]=true;queue.push(next);}
            }
        }
        let (bw,bh)=((maxx-minx+1) as f64,(maxy-miny+1) as f64);
        let (cx,cy)=(sx/area as f64,sy/area as f64);
        if area<12 || bw>48. || bh>30. || bw/bh>2.5 || bh/bw>2.5 || minx<2 || maxx>=w-2 || miny<2 || maxy>=h-2 {continue;}
        let compact=area as f64/(bw*bh);
        let candidate=area as f64*compact/(1.+((cy-h as f64/2.)/h as f64).abs());
        if compact>=0.4 && candidate>score {score=candidate;best=Some([cx/(w-1) as f64,cy/(h-1) as f64]);}
    }
    best.ok_or_else(||crate::invalid("Pupilla non distinguibile"))
}

pub(crate) fn pupils(value:Value)->io::Result<Value>{
    let eyes:Vec<Eye>=serde_json::from_value(value["eyes"].clone()).map_err(io::Error::other)?;
    if eyes.len()!=2 {return Err(crate::invalid("Seleziona due occhi"));}
    // A failed frame is a measurement result, not an application failure.
    match (pupil(&eyes[0]),pupil(&eyes[1])) {
        (Ok(left),Ok(right))=>Ok(json!({"valid":true,"eyes":[left,right],"position":[(left[0]+right[0])/2.,(left[1]+right[1])/2.]})),
        _=>Ok(json!({"valid":false})),
    }
}

#[derive(Clone,Deserialize)]
struct Sample { eye:[f64;2], target:[f64;2] }
fn terms(p:[f64;2],center:[f64;2],scale:[f64;2])->[f64;6]{
    let (x,y)=((p[0]-center[0])/scale[0],(p[1]-center[1])/scale[1]);[1.,x,y,x*x,x*y,y*y]
}
fn fit(samples:&[Sample],center:[f64;2],scale:[f64;2])->io::Result<[[f64;6];2]>{
    let mut a=[[0.;8];6];
    for s in samples {let t=terms(s.eye,center,scale);for i in 0..6 {for j in 0..6 {a[i][j]+=t[i]*t[j];}for k in 0..2 {a[i][6+k]+=t[i]*s.target[k];}}}
    for i in 0..6 {
        let pivot=(i..6).max_by(|&j,&k|a[j][i].abs().total_cmp(&a[k][i].abs())).unwrap();a.swap(i,pivot);
        if a[i][i].abs()<1e-8 {return Err(crate::invalid("Misure dello sguardo insufficienti"));}
        let divisor=a[i][i];for j in i..8 {a[i][j]/=divisor;}
        for row in 0..6 {if row==i {continue;}let factor=a[row][i];for j in i..8 {a[row][j]-=factor*a[i][j];}}
    }
    Ok(std::array::from_fn(|k|std::array::from_fn(|i|a[i][6+k])))
}
pub(crate) fn calibrate(value:Value)->io::Result<Value>{
    let samples:Vec<Sample>=serde_json::from_value(value["samples"].clone()).map_err(io::Error::other)?;
    if samples.len()<9 || samples.len()>144 || samples.iter().any(|s|s.eye.iter().any(|x|!x.is_finite() || !(0.0..=1.0).contains(x)) || s.target.iter().any(|x|!x.is_finite() || x.abs()>100000.)) {return Err(crate::invalid("Punti di calibrazione non validi"));}
    let center=std::array::from_fn(|k|samples.iter().map(|s|s.eye[k]).sum::<f64>()/samples.len() as f64);
    let scale=std::array::from_fn(|k|samples.iter().map(|s|(s.eye[k]-center[k]).powi(2)).sum::<f64>().sqrt()/(samples.len() as f64).sqrt());
    if scale.iter().any(|&v|v<0.012) {return Err(crate::invalid("Movimento delle pupille non distinguibile"));}
    let coefficients=fit(&samples,center,scale)?;
    let mut error=0.;
    for (index,s) in samples.iter().enumerate(){
        let others:Vec<Sample>=samples.iter().enumerate().filter(|(i,_)|*i!=index).map(|(_,s)|s.clone()).collect();
        let c=fit(&others,center,scale)?;let t=terms(s.eye,center,scale);
        error+=(0..2).map(|k|(0..6).map(|j|c[k][j]*t[j]).sum::<f64>()-s.target[k]).map(|d|d*d).sum::<f64>();
    }
    let error=(error/samples.len() as f64).sqrt();
    let extent=|k:usize|samples.iter().map(|s|s.target[k]).fold(f64::NEG_INFINITY,f64::max)-samples.iter().map(|s|s.target[k]).fold(f64::INFINITY,f64::min);
    if error>extent(0).hypot(extent(1))*0.15 {return Err(crate::invalid("Calibrazione imprecisa: ripeti mantenendo ferma la testa"));}
    Ok(json!({"center":center,"scale":scale,"coefficients":coefficients,"validation_error_px":error,"points":samples.len()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn dark_round_region_is_measured_but_blank_and_blink_are_rejected(){
        let mut eye=Eye{width:80,height:40,pixels:vec![190;3200]};
        assert!(pupil(&eye).is_err());
        for y in 0..40 {for x in 0..80 {if (x as i32-48).pow(2)+(y as i32-20).pow(2)<49 {eye.pixels[y*80+x]=20;}}}
        let p=pupil(&eye).unwrap();assert!((p[0]-48./79.).abs()<0.02 && (p[1]-20./39.).abs()<0.02);
        eye.pixels.fill(190);for y in 18..22 {for x in 4..76 {eye.pixels[y*80+x]=15;}}assert!(pupil(&eye).is_err());
        eye.width=usize::MAX;assert!(pupil(&eye).is_err());
    }
    #[test] fn fit_preserves_negative_desktop_coordinates_and_rejects_no_movement(){
        let samples:Vec<Value>=(0..3).flat_map(|y|(0..3).map(move|x|json!({"eye":[0.3+x as f64*0.15,0.3+y as f64*0.15],"target":[-1920.+x as f64*960.,-1080.+y as f64*540.]}))).collect();
        let model=calibrate(json!({"samples":samples})).unwrap();assert!(model["validation_error_px"].as_f64().unwrap()<1e-6);
        let center: [f64;2]=serde_json::from_value(model["center"].clone()).unwrap();let scale=serde_json::from_value(model["scale"].clone()).unwrap();let c:[[f64;6];2]=serde_json::from_value(model["coefficients"].clone()).unwrap();let t=terms([0.45,0.45],center,scale);
        assert!(((0..6).map(|i|c[0][i]*t[i]).sum::<f64>()+960.).abs()<1e-6);
        assert!(calibrate(json!({"samples":vec![json!({"eye":[0.5,0.5],"target":[0,0]});9]})).is_err());
    }
}
